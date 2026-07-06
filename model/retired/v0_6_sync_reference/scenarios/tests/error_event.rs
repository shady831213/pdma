#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, EventErrorKind, ModelError,
    desc::{CopyRequest, StridedCopyRequest},
};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn publishes_event_and_no_completion_when_linear_copy_reads_past_source() {
    // Given: a linear copy whose read range exceeds the source buffer.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4);
    let dst = driver.allocate_buffer_on(&mut handle, 8);

    // When: the device executes the invalid copy.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_error_after(0);
    let event = driver
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("error event was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the model returns the original error, publishes one event, and does not publish success.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(event.queue_id(), driver.queue_id());
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), src);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 8 }
    );
    assert_eq!(driver.poll_completion_on(&mut handle), Ok(None));
    assert_eq!(driver.poll_event_on(&mut handle), Ok(None));
}

#[test]
fn publishes_one_event_and_later_command_still_completes_after_bad_strided_copy() {
    // Given: a bad strided copy followed by a valid linear copy in the same queue.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let bad_src = driver.allocate_buffer_on(&mut handle, 8);
    let bad_dst = driver.allocate_buffer_on(&mut handle, 8);
    let good_src = driver.allocate_buffer_on(&mut handle, 4);
    let good_dst = driver.allocate_buffer_on(&mut handle, 4);
    driver
        .write_buffer_on(&mut handle, good_src, &[0x10, 0x11, 0x12, 0x13])
        .expect("source write is in bounds");

    // When: the first command fails and the caller runs the device again.
    let bad_command = driver
        .submit_strided_copy_on(
            &mut handle,
            StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
        )
        .expect("bad command has ring space");
    let good_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(good_src, good_dst, 4))
        .expect("good command has ring space");
    let error_snapshot = runner.wait_for_error_after(0);
    let event = driver
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("bad command event was published");
    let progress_snapshot = runner.wait_for_progress_after(0);
    let completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("good command completed");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the failed command produces one event, while the later command still completes.
    assert_eq!(error_snapshot.errors, 1);
    assert_eq!(progress_snapshot.steps, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(event.command_id(), bad_command);
    assert_eq!(event.buffer(), bad_src);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 8, end: 10 }
    );
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        driver.read_buffer_on(&handle, good_dst, 4),
        Ok(vec![0x10, 0x11, 0x12, 0x13])
    );
    assert_eq!(driver.poll_event_on(&mut handle), Ok(None));
}

#[test]
fn reclaims_driver_command_slot_when_error_event_is_polled() {
    // Given: a one-slot queue with one invalid command.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 1, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4);
    let dst = driver.allocate_buffer_on(&mut handle, 8);
    driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
        .expect("first command has ring space");
    assert_eq!(
        driver.submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1)),
        Err(ModelError::DriverQueueFull)
    );

    // When: the invalid command fails and the driver polls its error event.
    let snapshot = runner.wait_for_error_after(0);
    let event = driver
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("error event was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the event reclaims the command slot for this driver.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(event.queue_id(), driver.queue_id());
    assert!(
        driver
            .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1))
            .is_ok()
    );
}

#[test]
fn driver_event_polling_does_not_consume_another_queue_event() {
    // Given: one driver's queue has a pending memory fault event.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver_a = Driver::setup_on(&mut handle, 1, 4).expect("queue setup is valid");
    let mut driver_b = Driver::setup_on(&mut handle, 1, 4).expect("queue setup is valid");
    let src = driver_a.allocate_buffer_on(&mut handle, 4);
    let dst = driver_a.allocate_buffer_on(&mut handle, 8);
    let b_src = driver_b.allocate_buffer_on(&mut handle, 1);
    let b_dst = driver_b.allocate_buffer_on(&mut handle, 1);
    driver_a
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
        .expect("first driver command has ring space");
    let snapshot = runner.wait_for_error_after(0);

    // When: a different driver polls the device-level event path.
    let wrong_driver_event = driver_b
        .poll_event_on(&mut handle)
        .expect("event ring exists");
    let owner_event = driver_a
        .poll_event_on(&mut handle)
        .expect("event ring exists")
        .expect("owning driver can still poll its event");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the non-owning driver does not consume the event or reclaim its own slot.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(wrong_driver_event, None);
    assert_eq!(owner_event.queue_id(), driver_a.queue_id());
    assert!(
        driver_b
            .submit_copy_on(&mut handle, CopyRequest::new(b_src, b_dst, 1))
            .is_ok()
    );
    assert_eq!(
        driver_b.submit_copy_on(&mut handle, CopyRequest::new(b_src, b_dst, 1)),
        Err(ModelError::DriverQueueFull)
    );
}

#[test]
fn publishes_many_memory_fault_events_without_masking_original_error() {
    // Given: more invalid commands than the previous hidden event-ring depth.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 65, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4);
    let dst = driver.allocate_buffer_on(&mut handle, 8);
    for _ in 0..65 {
        driver
            .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
            .expect("command ring has enough space");
    }

    // When: every invalid command is executed without polling events between failures.
    let snapshot = runner.wait_for_error_after(64);
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: every fault kept its original error and produced a pollable event.
    assert_eq!(snapshot.errors, 65);
    assert_eq!(summary.errors, 65);
    for _ in 0..65 {
        let event = driver
            .poll_event_on(&mut handle)
            .expect("event polling succeeds")
            .expect("one event exists for each failed command");
        assert_eq!(
            event.error_kind(),
            EventErrorKind::OutOfBounds { len: 4, end: 8 }
        );
    }
    assert_eq!(driver.poll_event_on(&mut handle), Ok(None));
}
