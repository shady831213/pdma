#![allow(missing_docs)]

use pdma_modeling::desc::{CopyRequest, StridedCopyRequest};
use pdma_modeling::{CompletionStatus, Driver, EventErrorKind, ModelError, PdmaDevice};

#[test]
fn publishes_event_and_no_completion_when_linear_copy_reads_past_source() {
    // Given: a linear copy whose read range exceeds the source buffer.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 8);

    // When: the device executes the invalid copy.
    let command_id = driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 8))
        .expect("copy submission has ring space");
    let result = device.run_until_idle(1);
    let event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("error event was published");

    // Then: the model returns the original error, publishes one event, and does not publish success.
    assert_eq!(
        result,
        Err(ModelError::OutOfBounds {
            buffer: src,
            len: 4,
            end: 8,
        })
    );
    assert_eq!(event.queue_id(), driver.queue_id());
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), src);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 8 }
    );
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
    assert_eq!(driver.poll_event(&mut device), Ok(None));
}

#[test]
fn publishes_one_event_and_later_command_still_completes_after_bad_strided_copy() {
    // Given: a bad strided copy followed by a valid linear copy in the same queue.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let bad_src = driver.allocate_buffer(&mut device, 8);
    let bad_dst = driver.allocate_buffer(&mut device, 8);
    let good_src = driver.allocate_buffer(&mut device, 4);
    let good_dst = driver.allocate_buffer(&mut device, 4);
    driver
        .write_buffer(&mut device, good_src, &[0x10, 0x11, 0x12, 0x13])
        .expect("source write is in bounds");

    // When: the first command fails and the caller runs the device again.
    let bad_command = driver
        .submit_strided_copy(
            &mut device,
            StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
        )
        .expect("bad command has ring space");
    let good_command = driver
        .submit_copy(&mut device, CopyRequest::new(good_src, good_dst, 4))
        .expect("good command has ring space");
    let first_result = device.run_until_idle(1);
    let event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("bad command event was published");
    let second_summary = device
        .run_until_idle(1)
        .expect("later queued command remains runnable");
    let completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("good command completed");

    // Then: the failed command produces one event, while the later command still completes.
    assert_eq!(
        first_result,
        Err(ModelError::OutOfBounds {
            buffer: bad_src,
            len: 8,
            end: 10,
        })
    );
    assert_eq!(event.command_id(), bad_command);
    assert_eq!(event.buffer(), bad_src);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 8, end: 10 }
    );
    assert_eq!(second_summary.steps, 1);
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        driver.read_buffer(&device, good_dst, 4),
        Ok(vec![0x10, 0x11, 0x12, 0x13])
    );
    assert_eq!(driver.poll_event(&mut device), Ok(None));
}

#[test]
fn reclaims_driver_command_slot_when_error_event_is_polled() {
    // Given: a one-slot queue with one invalid command.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 1, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 8);
    driver
        .submit_copy(&mut device, CopyRequest::new(src, dst, 8))
        .expect("first command has ring space");
    assert_eq!(
        driver.submit_copy(&mut device, CopyRequest::new(src, dst, 1)),
        Err(ModelError::DriverQueueFull)
    );

    // When: the invalid command fails and the driver polls its error event.
    let result = device.run_until_idle(1);
    let event = driver
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("error event was published");

    // Then: the event reclaims the command slot for this driver.
    assert!(matches!(result, Err(ModelError::OutOfBounds { .. })));
    assert_eq!(event.queue_id(), driver.queue_id());
    assert!(
        driver
            .submit_copy(&mut device, CopyRequest::new(src, dst, 1))
            .is_ok()
    );
}

#[test]
fn driver_event_polling_does_not_consume_another_queue_event() {
    // Given: one driver's queue has a pending memory fault event.
    let mut device = PdmaDevice::new();
    let mut driver_a = Driver::setup(&mut device, 1, 4).expect("queue setup is valid");
    let mut driver_b = Driver::setup(&mut device, 1, 4).expect("queue setup is valid");
    let src = driver_a.allocate_buffer(&mut device, 4);
    let dst = driver_a.allocate_buffer(&mut device, 8);
    let b_src = driver_b.allocate_buffer(&mut device, 1);
    let b_dst = driver_b.allocate_buffer(&mut device, 1);
    driver_a
        .submit_copy(&mut device, CopyRequest::new(src, dst, 8))
        .expect("first driver command has ring space");
    assert!(matches!(
        device.run_until_idle(1),
        Err(ModelError::OutOfBounds { .. })
    ));

    // When: a different driver polls the device-level event path.
    let wrong_driver_event = driver_b.poll_event(&mut device).expect("event ring exists");
    let owner_event = driver_a
        .poll_event(&mut device)
        .expect("event ring exists")
        .expect("owning driver can still poll its event");

    // Then: the non-owning driver does not consume the event or reclaim its own slot.
    assert_eq!(wrong_driver_event, None);
    assert_eq!(owner_event.queue_id(), driver_a.queue_id());
    assert!(
        driver_b
            .submit_copy(&mut device, CopyRequest::new(b_src, b_dst, 1))
            .is_ok()
    );
    assert_eq!(
        driver_b.submit_copy(&mut device, CopyRequest::new(b_src, b_dst, 1)),
        Err(ModelError::DriverQueueFull)
    );
}

#[test]
fn publishes_many_memory_fault_events_without_masking_original_error() {
    // Given: more invalid commands than the previous hidden event-ring depth.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 65, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 4);
    let dst = driver.allocate_buffer(&mut device, 8);
    for _ in 0..65 {
        driver
            .submit_copy(&mut device, CopyRequest::new(src, dst, 8))
            .expect("command ring has enough space");
    }

    // When: every invalid command is executed without polling events between failures.
    for _ in 0..65 {
        assert_eq!(
            device.run_until_idle(1),
            Err(ModelError::OutOfBounds {
                buffer: src,
                len: 4,
                end: 8,
            })
        );
    }

    // Then: every fault kept its original error and produced a pollable event.
    for _ in 0..65 {
        let event = driver
            .poll_event(&mut device)
            .expect("event polling succeeds")
            .expect("one event exists for each failed command");
        assert_eq!(
            event.error_kind(),
            EventErrorKind::OutOfBounds { len: 4, end: 8 }
        );
    }
    assert_eq!(driver.poll_event(&mut device), Ok(None));
}
