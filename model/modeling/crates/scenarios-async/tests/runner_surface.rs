#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, desc::CopyRequest};
use pdma_model_device::PdmaDevice;
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_surface_records_errors_without_stopping_the_loop() {
    // Given: an async runner, a driver handle, one faulting command, and one later valid command.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let bad_src = driver.allocate_buffer_on(&mut handle, 4).await;
    let bad_dst = driver.allocate_buffer_on(&mut handle, 4).await;
    let good_src = driver.allocate_buffer_on(&mut handle, 4).await;
    let good_dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, good_src, &[7, 8, 9, 10])
        .await
        .expect("good source write is in bounds");

    // When: a bad command faults and a later good command still runs.
    let bad_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(bad_src, bad_dst, 8))
        .await
        .expect("faulting copy submission has ring space");
    let good_command = driver
        .submit_copy_on(&mut handle, CopyRequest::new(good_src, good_dst, 4))
        .await
        .expect("good copy submission has ring space");
    let error_snapshot = runner.wait_for_error_after(0).await;
    let fault = driver
        .poll_event_on(&mut handle)
        .await
        .expect("event ring exists")
        .expect("fault event was published");
    let progress_snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("good completion was published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the fault is recorded and the loop still retires the later good command.
    assert_eq!(fault.command_id(), bad_command);
    assert_eq!(completion.command_id(), good_command);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(error_snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(progress_snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(
        driver.read_buffer_on(&handle, good_dst, 4).await,
        Ok(vec![7, 8, 9, 10])
    );
}
