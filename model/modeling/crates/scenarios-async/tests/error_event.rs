#![allow(missing_docs)]

use pdma_model_contract::{EventErrorKind, desc::CopyRequest};
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
