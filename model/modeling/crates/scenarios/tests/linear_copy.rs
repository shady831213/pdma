#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, ModelError, desc::CopyRequest};
use pdma_model_device::{AsyncDevice, SyncDevice};
use pdma_model_host::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copies_bytes_end_to_end_when_driver_polls_completion() {
    // Given: async runner handle, AsyncDriver, source/destination buffers, source write.
    let runner = AsyncDevice::spawn_native();
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 5).await;
    let dst = driver.allocate_buffer_on(&mut handle, 5).await;
    driver
        .write_buffer_on(&mut handle, src, &[1, 2, 3, 4, 5])
        .await
        .expect("source write is in bounds");

    // When: submit copy via AsyncDriver, wait for progress, poll completion, shutdown/join.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 5))
        .await
        .expect("copy submission has ring space");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = driver
        .poll_completion_on(&mut handle)
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: completion command id/status match, snapshot/summary steps are 1, destination bytes equal source.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(
        driver.read_buffer_on(&handle, dst, 5).await,
        Ok(vec![1, 2, 3, 4, 5])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reports_step_budget_exhaustion_when_work_remains() {
    // Given: one pending copy command.
    let mut device = SyncDevice::new();
    let mut driver = AsyncDriver::setup_on(&mut device, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut device, 1).await;
    let dst = driver.allocate_buffer_on(&mut device, 1).await;
    driver
        .submit_copy_on(&mut device, CopyRequest::new(src, dst, 1))
        .await
        .expect("copy submission has ring space");

    // When: the device is given no steps.
    let result = device.run_until_idle(0);

    // Then: the pending command is reported as budget exhaustion.
    assert_eq!(
        result,
        Err(ModelError::StepBudgetExhausted { max_steps: 0 })
    );
}
