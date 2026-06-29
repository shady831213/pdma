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
    let driver = Driver::setup(&mut device, 4, 4)?;
    Ok((device, driver))
}

fn setup_with_completion_depth(
    config: ResourceConfig,
    completion_depth: usize,
) -> Result<(PdmaDevice, Driver), ModelError> {
    let mut device = PdmaDevice::with_resource_config(config);
    let driver = Driver::setup(&mut device, 4, completion_depth)?;
    Ok((device, driver))
}

#[test]
fn copy_stalls_without_read_slots_and_completes_with_one_read_slot() {
    // Given: a copy request and a device with no read slots.
    let (mut stalled_device, mut stalled_driver) =
        setup(ResourceConfig::default().with_read_slots(0)).expect("queue setup is valid");
    let stalled_src = stalled_driver.allocate_buffer(&mut stalled_device, 4);
    let stalled_dst = stalled_driver.allocate_buffer(&mut stalled_device, 4);
    stalled_driver
        .write_buffer(&mut stalled_device, stalled_src, &[1, 2, 3, 4])
        .expect("source write is in bounds");

    // When: software submits the copy and the device tries to retire it.
    stalled_driver
        .submit_copy(
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
    let (mut ready_device, mut ready_driver) =
        setup(ResourceConfig::default().with_read_slots(1)).expect("queue setup is valid");
    let ready_src = ready_driver.allocate_buffer(&mut ready_device, 4);
    let ready_dst = ready_driver.allocate_buffer(&mut ready_device, 4);
    ready_driver
        .write_buffer(&mut ready_device, ready_src, &[1, 2, 3, 4])
        .expect("source write is in bounds");

    // When: software submits the copy and the device retires it.
    let command_id = ready_driver
        .submit_copy(&mut ready_device, CopyRequest::new(ready_src, ready_dst, 4))
        .expect("copy submission has ring space");
    let summary = ready_device
        .run_until_idle(1)
        .expect("one step completes one copy");
    let completion = ready_driver
        .poll_completion(&mut ready_device)
        .expect("completion ring exists")
        .expect("copy published one completion");

    // Then: the copy completes successfully and the destination matches the source.
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        ready_driver.read_buffer(&ready_device, ready_dst, 4),
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
    let src = driver.allocate_buffer(&mut device, 8);
    let dst_a = driver.allocate_buffer(&mut device, 8);
    let dst_b = driver.allocate_buffer(&mut device, 8);
    driver
        .write_buffer(&mut device, src, &[0, 1, 2, 3, 4, 5, 6, 7])
        .expect("source write is in bounds");

    // When: software submits one broadcast command and the device tries to retire it.
    driver
        .submit_broadcast_copy(
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
    let src_a = driver.allocate_buffer(&mut device, 8);
    let src_b = driver.allocate_buffer(&mut device, 8);
    let dst = driver.allocate_buffer(&mut device, 8);

    // When: software submits the vector add and the device tries to retire it.
    driver
        .submit_vector_add(
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
    let src_a = driver.allocate_buffer(&mut device, 8);
    let src_b = driver.allocate_buffer(&mut device, 8);
    let dst = driver.allocate_buffer(&mut device, 8);

    // When: software submits the vector add and the device tries to retire it.
    driver
        .submit_vector_add(
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
    let (mut device, mut driver) = setup_with_completion_depth(
        ResourceConfig::default().with_completion_publication_slots(1),
        4,
    )
    .expect("queue setup is valid");
    let src_a = driver.allocate_buffer(&mut device, 4);
    let dst_a = driver.allocate_buffer(&mut device, 4);
    let src_b = driver.allocate_buffer(&mut device, 4);
    let dst_b = driver.allocate_buffer(&mut device, 4);
    driver
        .write_buffer(&mut device, src_a, &[10, 11, 12, 13])
        .expect("source A write is in bounds");
    driver
        .write_buffer(&mut device, src_b, &[20, 21, 22, 23])
        .expect("source B write is in bounds");

    // When: software submits both copies, fills completion publication, then polls before retrying.
    let first_command = driver
        .submit_copy(&mut device, CopyRequest::new(src_a, dst_a, 4))
        .expect("first copy submission has ring space");
    let second_command = driver
        .submit_copy(&mut device, CopyRequest::new(src_b, dst_b, 4))
        .expect("second copy submission has ring space");
    let first_step = device
        .step()
        .expect("first copy retires before the ring fills");
    let stalled = device.run_until_idle(1);
    let first_completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("first completion was published");
    let second_summary = device
        .run_until_idle(1)
        .expect("second copy retires after the first completion is polled");
    let second_completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("second completion was published");

    // Then: retirement stays ordered across polling.
    assert!(first_step);
    assert_eq!(first_completion.command_id(), first_command);
    assert_eq!(first_completion.status(), CompletionStatus::Success);
    assert_eq!(
        stalled,
        Err(ModelError::StepBudgetExhausted { max_steps: 1 })
    );
    assert_eq!(second_summary.steps, 1);
    assert_eq!(second_completion.command_id(), second_command);
    assert_eq!(second_completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
}

#[test]
fn event_capacity_one_preserves_event_order_after_polling() {
    // Given: two faulting copies behind a one-deep event publication path.
    let (mut device, mut driver) = setup(ResourceConfig::default().with_event_publication_slots(1))
        .expect("queue setup is valid");
    let bad_src_a = driver.allocate_buffer(&mut device, 4);
    let bad_dst_a = driver.allocate_buffer(&mut device, 8);
    let bad_src_b = driver.allocate_buffer(&mut device, 4);
    let bad_dst_b = driver.allocate_buffer(&mut device, 8);

    // When: software submits both invalid copies, fills event publication, then polls before retrying.
    let first_command = driver
        .submit_copy(&mut device, CopyRequest::new(bad_src_a, bad_dst_a, 8))
        .expect("first faulting copy has ring space");
    let second_command = driver
        .submit_copy(&mut device, CopyRequest::new(bad_src_b, bad_dst_b, 8))
        .expect("second faulting copy has ring space");
    let first_result = device.run_until_idle(1);
    let stalled = device.run_until_idle(1);
    let first_event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("first error event was published");
    let second_result = device.run_until_idle(1);
    let second_event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("second error event was published");

    // Then: error publication stays ordered across polling.
    assert!(matches!(first_result, Err(ModelError::OutOfBounds { .. })));
    assert_eq!(first_event.command_id(), first_command);
    assert_eq!(
        stalled,
        Err(ModelError::StepBudgetExhausted { max_steps: 1 })
    );
    assert!(matches!(second_result, Err(ModelError::OutOfBounds { .. })));
    assert_eq!(second_event.command_id(), second_command);
    assert_eq!(driver.poll_event(&mut device), Ok(None));
}
