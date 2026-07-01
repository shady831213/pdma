#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, desc::StridedCopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copies_rows_end_to_end_when_strides_include_padding() {
    // Given: async runner handle, AsyncDriver, padded source rows, and sentinel destination bytes.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 16).await;
    let dst = driver.allocate_buffer_on(&mut handle, 20).await;
    driver
        .write_buffer_on(
            &mut handle,
            src,
            &[1, 2, 3, 9, 4, 5, 6, 9, 7, 8, 9, 9, 0, 0, 0, 0],
        )
        .await
        .expect("source write is in bounds");
    driver
        .write_buffer_on(&mut handle, dst, &[0xEE; 20])
        .await
        .expect("destination write is in bounds");

    // When: submit strided copy via AsyncDriver, wait for progress, poll completion, shutdown/join.
    let command_id = driver
        .submit_strided_copy_on(&mut handle, StridedCopyRequest::new(src, dst, 3, 3, 4, 6))
        .await
        .expect("strided copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: row payloads are copied, destination padding stays unchanged, and one completion is published.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.queue_id(), driver.queue_id());
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 20).await,
        Ok(vec![
            1, 2, 3, 0xEE, 0xEE, 0xEE, 4, 5, 6, 0xEE, 0xEE, 0xEE, 7, 8, 9, 0xEE, 0xEE, 0xEE, 0xEE,
            0xEE,
        ])
    );
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_reports_out_of_bounds_when_strided_row_exceeds_source_buffer() {
    // Given: async runner handle, AsyncDriver, and a strided copy whose second row is too long.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 8).await;
    let dst = driver.allocate_buffer_on(&mut handle, 8).await;
    let request = StridedCopyRequest::new(src, dst, 2, 4, 6, 4);
    driver
        .submit_strided_copy_on(&mut handle, request)
        .await
        .expect("strided copy submission has ring space");

    // When: the async runner executes the command.
    let snapshot = runner.wait_for_error_after(0).await;
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: checked memory reports the failing row and no successful completion is published.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}
