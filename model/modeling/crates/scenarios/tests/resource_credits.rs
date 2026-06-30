#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, ModelError, ResourceConfig,
    desc::{
        BroadcastCopyRequest, BroadcastDestinationRange, CopyRequest, VectorAddRequest,
        VectorBufferRange,
    },
};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

fn setup(config: ResourceConfig) -> Result<(PdmaDevice, Driver), ModelError> {
    let mut device = PdmaDevice::with_resource_config(config);
    let driver = Driver::setup_on(&mut device, 4, 4)?;
    Ok((device, driver))
}

#[test]
fn copy_stalls_without_read_slots_and_completes_with_one_read_slot() {
    // Given: a copy request and a device with no read slots.
    let (mut stalled_device, mut stalled_driver) =
        setup(ResourceConfig::default().with_read_slots(0)).expect("queue setup is valid");
    let stalled_src = stalled_driver.allocate_buffer_on(&mut stalled_device, 4);
    let stalled_dst = stalled_driver.allocate_buffer_on(&mut stalled_device, 4);
    stalled_driver
        .write_buffer_on(&mut stalled_device, stalled_src, &[1, 2, 3, 4])
        .expect("source write is in bounds");

    // When: software submits the copy and the device tries to retire it.
    stalled_driver
        .submit_copy_on(
            &mut stalled_device,
            CopyRequest::new(stalled_src, stalled_dst, 4),
        )
        .expect("copy submission has ring space");
    let stalled_result = stalled_device.run_until_idle(1);

    // Then: the command stays pending because the read slot budget is empty.
    assert_eq!(
        stalled_result,
        Err(ModelError::StepBudgetExhausted { max_steps: 1 })
    );

    // Given: the same copy shape but with one read slot available.
    let ready_device =
        PdmaDevice::with_resource_config(ResourceConfig::default().with_read_slots(1));
    let runner = ready_device
        .spawn_sync_runner()
        .expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut ready_driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let ready_src = ready_driver.allocate_buffer_on(&mut handle, 4);
    let ready_dst = ready_driver.allocate_buffer_on(&mut handle, 4);
    ready_driver
        .write_buffer_on(&mut handle, ready_src, &[1, 2, 3, 4])
        .expect("source write is in bounds");

    // When: software submits the copy and the device retires it.
    let command_id = ready_driver
        .submit_copy_on(&mut handle, CopyRequest::new(ready_src, ready_dst, 4))
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0);
    let completion = ready_driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("copy published one completion");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the copy completes successfully and the destination matches the source.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        ready_driver.read_buffer_on(&handle, ready_dst, 4),
        Ok(vec![1, 2, 3, 4])
    );
}

#[test]
fn broadcast_stalls_when_write_and_descriptor_credit_is_below_demand() {
    // Given: one source and two destinations on a device with insufficient write and descriptor credit.
    let (mut device, mut driver) = setup(
        ResourceConfig::default()
            .with_write_slots(1)
            .with_internal_descriptors(1),
    )
    .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut device, 8);
    let dst_a = driver.allocate_buffer_on(&mut device, 8);
    let dst_b = driver.allocate_buffer_on(&mut device, 8);
    driver
        .write_buffer_on(&mut device, src, &[0, 1, 2, 3, 4, 5, 6, 7])
        .expect("source write is in bounds");

    // When: software submits one broadcast command and the device tries to retire it.
    driver
        .submit_broadcast_copy_on(
            &mut device,
            BroadcastCopyRequest::new(
                src,
                2,
                4,
                vec![
                    BroadcastDestinationRange::new(dst_a, 0),
                    BroadcastDestinationRange::new(dst_b, 0),
                ],
            ),
        )
        .expect("broadcast submission has ring space");
    let result = device.run_until_idle(1);

    // Then: the command stalls instead of retiring with too little write or descriptor credit.
    assert_eq!(
        result,
        Err(ModelError::StepBudgetExhausted { max_steps: 1 })
    );
}

#[test]
fn vector_add_stalls_with_fewer_than_two_read_slots() {
    // Given: a vector-add command on a device with only one read slot.
    let (mut device, mut driver) =
        setup(ResourceConfig::default().with_read_slots(1)).expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut device, 8);
    let src_b = driver.allocate_buffer_on(&mut device, 8);
    let dst = driver.allocate_buffer_on(&mut device, 8);

    // When: software submits the vector add and the device tries to retire it.
    driver
        .submit_vector_add_on(
            &mut device,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 0),
                VectorBufferRange::new(src_b, 0),
                VectorBufferRange::new(dst, 0),
                4,
            ),
        )
        .expect("vector-add submission has ring space");
    let result = device.run_until_idle(1);

    // Then: the command stalls because it needs two reads.
    assert_eq!(
        result,
        Err(ModelError::StepBudgetExhausted { max_steps: 1 })
    );
}

#[test]
fn vector_add_stalls_with_zero_compute_slots() {
    // Given: a vector-add command on a device with no compute slots.
    let (mut device, mut driver) =
        setup(ResourceConfig::default().with_compute_slots(0)).expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut device, 8);
    let src_b = driver.allocate_buffer_on(&mut device, 8);
    let dst = driver.allocate_buffer_on(&mut device, 8);

    // When: software submits the vector add and the device tries to retire it.
    driver
        .submit_vector_add_on(
            &mut device,
            VectorAddRequest::new(
                VectorBufferRange::new(src_a, 0),
                VectorBufferRange::new(src_b, 0),
                VectorBufferRange::new(dst, 0),
                4,
            ),
        )
        .expect("vector-add submission has ring space");
    let result = device.run_until_idle(1);

    // Then: the command stalls because no compute slot is available.
    assert_eq!(
        result,
        Err(ModelError::StepBudgetExhausted { max_steps: 1 })
    );
}

#[test]
fn completion_publication_capacity_one_preserves_completion_order_after_polling() {
    // Given: two copies queued behind a one-slot completion publication path.
    let device = PdmaDevice::with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut handle, 4);
    let dst_a = driver.allocate_buffer_on(&mut handle, 4);
    let src_b = driver.allocate_buffer_on(&mut handle, 4);
    let dst_b = driver.allocate_buffer_on(&mut handle, 4);
    driver
        .write_buffer_on(&mut handle, src_a, &[10, 11, 12, 13])
        .expect("source A write is in bounds");
    driver
        .write_buffer_on(&mut handle, src_b, &[20, 21, 22, 23])
        .expect("source B write is in bounds");

    // When: software submits both copies, fills completion publication, then polls before retrying.
    let first_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 4))
        .expect("first copy submission has ring space");
    let second_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 4))
        .expect("second copy submission has ring space");
    let first_snapshot = runner.wait_for_progress_after(0);
    let first_completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("first completion was published");
    let second_snapshot = runner.wait_for_progress_after(first_snapshot.steps);
    let second_completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("second completion was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: retirement stays ordered across polling.
    assert_eq!(first_snapshot.steps, 1);
    assert_eq!(first_completion.command_id(), first_command);
    assert_eq!(first_completion.status(), CompletionStatus::Success);
    assert!(second_snapshot.stalls >= 1);
    assert_eq!(second_snapshot.steps, 2);
    assert_eq!(summary.steps, 2);
    assert_eq!(second_completion.command_id(), second_command);
    assert_eq!(second_completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion_on(&mut handle), Ok(None));
}

#[test]
fn event_capacity_one_preserves_event_order_after_polling() {
    // Given: two faulting copies behind a one-deep event publication path.
    let device =
        PdmaDevice::with_resource_config(ResourceConfig::default().with_event_publication_slots(1));
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let bad_src_a = driver.allocate_buffer_on(&mut handle, 4);
    let bad_dst_a = driver.allocate_buffer_on(&mut handle, 8);
    let bad_src_b = driver.allocate_buffer_on(&mut handle, 4);
    let bad_dst_b = driver.allocate_buffer_on(&mut handle, 8);

    // When: software submits both invalid copies, fills event publication, then polls before retrying.
    let first_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(bad_src_a, bad_dst_a, 8))
        .expect("first faulting copy has ring space");
    let second_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(bad_src_b, bad_dst_b, 8))
        .expect("second faulting copy has ring space");
    let first_snapshot = runner.wait_for_error_after(0);
    let first_event = driver
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("first error event was published");
    let second_snapshot = runner.wait_for_error_after(first_snapshot.errors);
    let second_event = driver
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("second error event was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: error publication stays ordered across polling.
    assert_eq!(first_snapshot.errors, 1);
    assert_eq!(first_event.command_id(), first_command);
    assert!(second_snapshot.stalls >= 1);
    assert_eq!(second_snapshot.errors, 2);
    assert_eq!(summary.errors, 2);
    assert_eq!(second_event.command_id(), second_command);
    assert_eq!(driver.poll_event_on(&mut handle), Ok(None));
}
