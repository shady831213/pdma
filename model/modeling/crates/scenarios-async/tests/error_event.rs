#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, EventErrorKind,
    desc::{CopyRequest, StridedCopyRequest},
};
use pdma_model_device::PdmaDevice;
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_event_and_no_completion_when_linear_copy_reads_past_source() {
    // Given: async runner handle, AsyncDriver, and a linear copy whose source read is too long.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 8).await;

    // When: the async runner executes the invalid copy and software polls its event.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
        .await
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_error_after(0).await;
    let event = driver
        .poll_event_on(&mut handle)
        .await
        .expect("event ring exists")
        .expect("error event was published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the original out-of-bounds event is published and no success completion appears.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(event.queue_id(), driver.queue_id());
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), src);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 8 }
    );
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
    assert_eq!(driver.poll_event_on(&mut handle).await, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_one_event_and_later_command_still_completes_after_bad_strided_copy()
{
    // Given: async runner handle, AsyncDriver, and a bad strided copy before a valid linear copy.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let bad_src = driver.allocate_buffer_on(&mut handle, 8).await;
    let bad_dst = driver.allocate_buffer_on(&mut handle, 8).await;
    let good_src = driver.allocate_buffer_on(&mut handle, 4).await;
    let good_dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, good_src, &[0x10, 0x11, 0x12, 0x13])
        .await
        .expect("source write is in bounds");

    // When: the first command fails and the async runner later completes the valid command.
    let bad_command = driver
        .submit_strided_copy_on(
            &mut handle,
            StridedCopyRequest::new(bad_src, bad_dst, 2, 4, 6, 4),
        )
        .await
        .expect("bad command has ring space");
    let good_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(good_src, good_dst, 4))
        .await
        .expect("good command has ring space");
    let error_snapshot = runner.wait_for_error_after(0).await;
    let event = driver
        .poll_event_on(&mut handle)
        .await
        .expect("event ring exists")
        .expect("bad command event was published");
    let progress_snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("good command completed");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the failed command publishes one event, while the later command still completes.
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
        driver.read_buffer_on(&handle, good_dst, 4).await,
        Ok(vec![0x10, 0x11, 0x12, 0x13])
    );
    assert_eq!(driver.poll_event_on(&mut handle).await, Ok(None));
}
