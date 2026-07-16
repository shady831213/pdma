#![allow(missing_docs)]

use pdma_model_contract::{CompletionStatus, ModelError, desc::CopyRequest};
use pdma_model_device::{AsyncDevice, SyncDevice};
use pdma_model_host::{AsyncDriver, SoftwareCpu};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copies_bytes_end_to_end_when_driver_polls_completion() {
    // Given: async runner handle, AsyncDriver, source/destination buffers, source write.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let cpu = SoftwareCpu::new();
    let (completion, command_id, snapshot, dst_bytes) = cpu
        .run_main(async {
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

            // When: submit copy via AsyncDriver, wait for progress, and poll completion.
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
            let dst_bytes = driver.read_buffer_on(&handle, dst, 5).await;
            (completion, command_id, snapshot, dst_bytes)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: completion, destination bytes, and worker summary report one successful copy.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(dst_bytes, Ok(vec![1, 2, 3, 4, 5]));
    assert_eq!(summary.steps, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn reports_step_budget_exhaustion_when_work_remains() {
    // Given: one pending copy command.
    let mut device = SyncDevice::new();
    let mut driver = AsyncDriver::setup_manual_on(&mut device, 4, 4)
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
