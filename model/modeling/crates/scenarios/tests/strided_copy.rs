#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, desc::StridedCopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_host::Driver;

#[test]
fn copies_rows_end_to_end_when_strides_include_padding() {
    // Given: a source image with row padding and a destination initialized with sentinels.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 16);
    let dst = driver.allocate_buffer_on(&mut handle, 20);
    driver
        .write_buffer_on(
            &mut handle,
            src,
            &[1, 2, 3, 9, 4, 5, 6, 9, 7, 8, 9, 9, 0, 0, 0, 0],
        )
        .expect("source write is in bounds");
    driver
        .write_buffer_on(&mut handle, dst, &[0xEE; 20])
        .expect("destination write is in bounds");

    // When: software submits a 3-row strided copy and the device runs one step.
    let command_id = driver
        .submit_strided_copy_on(&mut handle, StridedCopyRequest::new(src, dst, 3, 3, 4, 6))
        .expect("strided copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0);
    let completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: row payloads are copied, destination padding remains untouched, and one completion is published.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.queue_id(), driver.queue_id());
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 20),
        Ok(vec![
            1, 2, 3, 0xEE, 0xEE, 0xEE, 4, 5, 6, 0xEE, 0xEE, 0xEE, 7, 8, 9, 0xEE, 0xEE, 0xEE, 0xEE,
            0xEE,
        ])
    );
    assert_eq!(driver.poll_completion_on(&mut handle), Ok(None));
}

#[test]
fn reports_out_of_bounds_when_strided_row_exceeds_source_buffer() {
    // Given: a strided copy whose second row runs past the source buffer.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 8);
    let dst = driver.allocate_buffer_on(&mut handle, 8);
    let request = StridedCopyRequest::new(src, dst, 2, 4, 6, 4);
    driver
        .submit_strided_copy_on(&mut handle, request)
        .expect("strided copy submission has ring space");

    // When: the device executes the command.
    let snapshot = runner.wait_for_error_after(0);
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: checked memory reports the failing row and no successful completion is published.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(driver.poll_completion_on(&mut handle), Ok(None));
}

#[test]
fn keeps_queue_runnable_when_failed_strided_copy_has_later_work() {
    // Given: a bad strided copy followed by a valid strided copy in the same queue.
    let device = PdmaDevice::new();
    let runner = device.spawn_sync_runner().expect("sync runner spawns");
    let mut handle = runner.handle();
    let mut driver = Driver::setup_on(&mut handle, 4, 4).expect("queue setup is valid");
    let bad_src = driver.allocate_buffer_on(&mut handle, 8);
    let good_src = driver.allocate_buffer_on(&mut handle, 4);
    let bad_dst = driver.allocate_buffer_on(&mut handle, 8);
    let good_dst = driver.allocate_buffer_on(&mut handle, 4);
    driver
        .write_buffer_on(&mut handle, good_src, &[0xA0, 0xA1, 0xA2, 0xA3])
        .expect("source write is in bounds");
    driver
        .submit_strided_copy_on(
            &mut handle,
            StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
        )
        .expect("bad command has ring space");
    let good_command = driver
        .submit_strided_copy_on(
            &mut handle,
            StridedCopyRequest::new(good_src, good_dst, 1, 4, 4, 4),
        )
        .expect("good command has ring space");

    // When: the first step fails and the caller runs the device again.
    let error_snapshot = runner.wait_for_error_after(0);
    let progress_snapshot = runner.wait_for_progress_after(0);
    let completion = driver
        .poll_completion_on(&mut handle)
        .expect("completion ring exists")
        .expect("good command completed");
    runner.request_shutdown();
    let summary = runner.join().expect("runner joins cleanly");

    // Then: the bad command reports its error, while the later command still completes.
    assert_eq!(error_snapshot.errors, 1);
    assert_eq!(progress_snapshot.steps, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(
        driver.read_buffer_on(&handle, good_dst, 4),
        Ok(vec![0xA0, 0xA1, 0xA2, 0xA3])
    );
}
