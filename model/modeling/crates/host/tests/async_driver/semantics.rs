use pdma_model_contract::{BufferId, CommandId, CopyRequest, ModelError};
use pdma_model_device::{AsyncDevice, AsyncStepStatus, SyncDevice};
use pdma_model_host::{AsyncDriver, ManualDriver, SoftwareCpu};

#[tokio::test(flavor = "current_thread")]
async fn completion_pop_reclaims_credit_before_future_can_suspend() {
    // Given: a completed command on a depth-one CPU-bound driver.
    let mut device = SyncDevice::new();

    // When: software pops the completion and immediately submits another command.
    let (completion, next) = SoftwareCpu::new()
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut device, 1, 1).await?;
            let src = driver
                .allocate_buffer_on(&mut device, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut device, 4)
                .await
                .expect("allocation");
            driver
                .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
                .await?;
            let summary = device.run_until_idle(8)?;
            assert_eq!(summary.last_status, AsyncStepStatus::Idle);
            let completion = driver.poll_completion_on(&mut device).await?;
            let next = driver
                .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
                .await;
            Ok::<_, ModelError>((completion, next))
        })
        .await
        .expect("manual execution remains valid");

    // Then: the reclaimed credit is visible in the same CPU poll.
    assert_eq!(
        completion.map(pdma_model_contract::CompletionRecord::command_id),
        Some(CommandId::new(0))
    );
    assert_eq!(next, Ok(CommandId::new(1)));
}

#[tokio::test(flavor = "current_thread")]
async fn event_pop_reclaims_credit_before_future_can_suspend() {
    // Given: a fault event for a depth-one CPU-bound driver.
    let (device, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut port = device.handle();

    // When: software pops the event and immediately submits another command.
    let (event, next) = SoftwareCpu::new()
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let dst = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("allocation");
            driver
                .submit_copy_on(&mut port, CopyRequest::new(BufferId::new(99), dst, 4))
                .await?;
            let _snapshot = device.wait_for_error_after(0).await;
            let event = driver.poll_event_on(&mut port).await?;
            let next = driver
                .submit_copy_on(&mut port, CopyRequest::new(dst, dst, 4))
                .await;
            Ok::<_, ModelError>((event, next))
        })
        .await
        .expect("manual execution remains valid");
    device.request_shutdown().await;
    let _summary = worker.await.expect("worker joins cleanly");

    // Then: event handling restores the shared command credit synchronously.
    assert_eq!(
        event.map(pdma_model_contract::EventRecord::command_id),
        Some(CommandId::new(0))
    );
    assert_eq!(next, Ok(CommandId::new(1)));
}

#[tokio::test(flavor = "current_thread")]
async fn manual_mode_matches_shared_credit_and_reclaim_semantics() {
    // Given: two manual driver capabilities for one depth-one queue.
    let mut device = SyncDevice::new();
    let mut first = ManualDriver::setup_on(&mut device, 1, 1)
        .await
        .expect("manual setup is valid");
    let mut second = first.clone();
    let src = first
        .allocate_buffer_on(&mut device, 4)
        .await
        .expect("allocation");
    let dst = first
        .allocate_buffer_on(&mut device, 4)
        .await
        .expect("allocation");

    // When: one clone consumes, the peer observes full, then reclaims the completion.
    let submitted = first
        .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
        .await;
    let full = second
        .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
        .await;
    assert_eq!(
        device.run_until_idle(8).map(|summary| summary.last_status),
        Ok(AsyncStepStatus::Idle)
    );
    let completion = second.poll_completion_on(&mut device).await;
    let retried = first
        .submit_copy_on(&mut device, CopyRequest::new(src, dst, 4))
        .await;

    // Then: manual mode uses the same canonical accounting allocation.
    assert_eq!(submitted, Ok(CommandId::new(0)));
    assert_eq!(full, Err(ModelError::DriverQueueFull));
    assert!(matches!(completion, Ok(Some(_))));
    assert_eq!(retried, Ok(CommandId::new(1)));
}
