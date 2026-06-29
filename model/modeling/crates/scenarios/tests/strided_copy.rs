#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, ModelError, desc::StridedCopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn copies_rows_end_to_end_when_strides_include_padding() {
    // Given: a source image with row padding and a destination initialized with sentinels.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 16);
    let dst = driver.allocate_buffer(&mut device, 20);
    device
        .write_buffer(src, 0, &[1, 2, 3, 9, 4, 5, 6, 9, 7, 8, 9, 9, 0, 0, 0, 0])
        .expect("source write is in bounds");
    device
        .write_buffer(dst, 0, &[0xEE; 20])
        .expect("destination write is in bounds");

    // When: software submits a 3-row strided copy and the device runs one step.
    let command_id = driver
        .submit_strided_copy(&mut device, StridedCopyRequest::new(src, dst, 3, 3, 4, 6))
        .expect("strided copy submission has ring space");
    let summary = device
        .run_until_idle(1)
        .expect("one step completes one strided copy");
    let completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("completion was published");

    // Then: row payloads are copied, destination padding remains untouched, and one completion is published.
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.queue_id(), driver.queue_id());
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        device.read_buffer(dst, 0, 20),
        Ok(vec![
            1, 2, 3, 0xEE, 0xEE, 0xEE, 4, 5, 6, 0xEE, 0xEE, 0xEE, 7, 8, 9, 0xEE, 0xEE, 0xEE, 0xEE,
            0xEE,
        ])
    );
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
}

#[test]
fn reports_out_of_bounds_when_strided_row_exceeds_source_buffer() {
    // Given: a strided copy whose second row runs past the source buffer.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer(&mut device, 8);
    let dst = driver.allocate_buffer(&mut device, 8);
    let request = StridedCopyRequest::new(src, dst, 2, 4, 6, 4);
    driver
        .submit_strided_copy(&mut device, request)
        .expect("strided copy submission has ring space");

    // When: the device executes the command.
    let result = device.run_until_idle(1);

    // Then: checked memory reports the failing row and no successful completion is published.
    assert_eq!(
        result,
        Err(ModelError::OutOfBounds {
            buffer: src,
            len: 8,
            end: 10,
        })
    );
    assert_eq!(driver.poll_completion(&mut device), Ok(None));
}

#[test]
fn keeps_queue_runnable_when_failed_strided_copy_has_later_work() {
    // Given: a bad strided copy followed by a valid strided copy in the same queue.
    let mut device = PdmaDevice::new();
    let mut driver = Driver::setup(&mut device, 4, 4).expect("queue setup is valid");
    let bad_src = driver.allocate_buffer(&mut device, 8);
    let good_src = driver.allocate_buffer(&mut device, 4);
    let bad_dst = driver.allocate_buffer(&mut device, 8);
    let good_dst = driver.allocate_buffer(&mut device, 4);
    device
        .write_buffer(good_src, 0, &[0xA0, 0xA1, 0xA2, 0xA3])
        .expect("source write is in bounds");
    driver
        .submit_strided_copy(
            &mut device,
            StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
        )
        .expect("bad command has ring space");
    let good_command = driver
        .submit_strided_copy(
            &mut device,
            StridedCopyRequest::new(good_src, good_dst, 1, 4, 4, 4),
        )
        .expect("good command has ring space");

    // When: the first step fails and the caller runs the device again.
    let first_result = device.run_until_idle(1);
    let second_summary = device
        .run_until_idle(1)
        .expect("later queued command remains runnable");
    let completion = driver
        .poll_completion(&mut device)
        .expect("completion ring exists")
        .expect("good command completed");

    // Then: the bad command reports its error, while the later command still completes.
    assert_eq!(
        first_result,
        Err(ModelError::OutOfBounds {
            buffer: bad_src,
            len: 8,
            end: 10,
        })
    );
    assert_eq!(second_summary.steps, 1);
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(
        device.read_buffer(good_dst, 0, 4),
        Ok(vec![0xA0, 0xA1, 0xA2, 0xA3])
    );
}
