#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, ResourceConfig, desc::CopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_completion_publication_capacity_one_preserves_completion_order_after_polling()
{
    // Given: two async copies queued behind a one-slot completion publication path.
    let device = PdmaDevice::with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src_a = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst_a = driver.allocate_buffer_on(&mut handle, 4).await;
    let src_b = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst_b = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, src_a, &[10, 11, 12, 13])
        .await
        .expect("source A write is in bounds");
    driver
        .write_buffer_on(&mut handle, src_b, &[20, 21, 22, 23])
        .await
        .expect("source B write is in bounds");

    // When: software submits both copies, fills completion publication, then polls before retrying.
    let first_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 4))
        .await
        .expect("first copy submission has ring space");
    let second_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 4))
        .await
        .expect("second copy submission has ring space");
    let first_snapshot = runner.wait_for_progress_after(0).await;
    let first_completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("first completion was published");
    let second_snapshot = runner.wait_for_progress_after(first_snapshot.steps).await;
    let second_completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("second completion was published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: retirement stays ordered across polling.
    assert_eq!(first_snapshot.steps, 1);
    assert_eq!(first_completion.command_id(), first_command);
    assert_eq!(first_completion.status(), CompletionStatus::Success);
    assert!(second_snapshot.stalls >= 1);
    assert_eq!(second_snapshot.steps, 2);
    assert_eq!(summary.steps, 2);
    assert_eq!(second_completion.command_id(), second_command);
    assert_eq!(second_completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
}
