#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus,
    desc::{BroadcastCopyRequest, BroadcastDestinationRange},
};
use pdma_model_device::PdmaDevice;
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_broadcasts_one_source_range_to_multiple_destinations_and_publishes_one_completion()
 {
    // Given: async runner handle, AsyncDriver, one source range, and two destination ranges.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 2, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 8).await;
    let dst_a = driver.allocate_buffer_on(&mut handle, 8).await;
    let dst_b = driver.allocate_buffer_on(&mut handle, 8).await;
    driver
        .write_buffer_on(&mut handle, src, &[0, 1, 2, 3, 4, 5, 6, 7])
        .await
        .expect("source write is in bounds");

    // When: submit broadcast copy via AsyncDriver, wait for progress, poll completion, shutdown/join.
    let command_id = driver
        .submit_broadcast_copy_on(
            &mut handle,
            BroadcastCopyRequest::new(
                src,
                2,
                4,
                vec![
                    BroadcastDestinationRange::new(dst_a, 0),
                    BroadcastDestinationRange::new(dst_b, 3),
                ],
            ),
        )
        .await
        .expect("broadcast submission has ring space");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("broadcast published one completion");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: both destinations receive the same source bytes and only one completion appears.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(driver.poll_completion_on(&mut handle).await, Ok(None));
    assert_eq!(
        driver.read_buffer_on(&handle, dst_a, 8).await,
        Ok(vec![2, 3, 4, 5, 0, 0, 0, 0])
    );
    assert_eq!(
        driver.read_buffer_on(&handle, dst_b, 8).await,
        Ok(vec![0, 0, 0, 2, 3, 4, 5, 0])
    );
}
