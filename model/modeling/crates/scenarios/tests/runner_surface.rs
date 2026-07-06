#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, desc::CopyRequest};
use pdma_model_device::AsyncDevice;
use pdma_model_host::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_surface_interleaves_software_and_hardware_turns() {
    // Given: an async runner, a runner handle, and source/destination buffers.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
        .await
        .expect("source write is in bounds");

    // When: software submits a copy, waits on the async runner, and then polls completion.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the async runner-backed surface preserves the copy outcome across turns.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 4).await,
        Ok(vec![1, 2, 3, 4])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_surface_poll_before_progress_observes_no_completion() {
    // Given: an async runner, a runner handle, and source/destination buffers.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
        .await
        .expect("source write is in bounds");

    // When: software submits a copy, polls before progress, then waits for hardware progress.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    let early_completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the early software poll sees no completion, and the later poll observes success.
    assert_eq!(early_completion, None);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 4).await,
        Ok(vec![1, 2, 3, 4])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_surface_records_errors_without_stopping_the_loop() {
    // Given: an async runner, a driver handle, one faulting command, and one later valid command.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
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
    let summary = worker.await.expect("runner joins cleanly");

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

#[tokio::test(flavor = "current_thread")]
async fn async_runner_surface_reports_a_shutdown_and_join_summary() {
    // Given: an async runner and one completed command.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;
    driver
        .write_buffer_on(&mut handle, src, &[1, 3, 5, 7])
        .await
        .expect("source write is in bounds");

    // When: software submits a copy, waits once, and then shuts the async runner down.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: shutdown produces a join summary after the successful copy.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 4).await,
        Ok(vec![1, 3, 5, 7])
    );
}
