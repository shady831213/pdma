use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::CommandId;
use pdma_model_device::AsyncDevice;
use pdma_model_host::CopyRequest;
use pdma_model_host::DriverError;
use pdma_model_host::{AsyncDriver, ManualDriver};

#[tokio::test(flavor = "current_thread")]
async fn completion_pop_reclaims_credit_before_future_can_suspend() {
    // Given: a completed command on a depth-one CPU-bound driver.
    let storage = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new(
        memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let mut port = super::support::BlockingPort::new(device.handle(), memory.clone());

    // When: software pops the completion and immediately submits another command.
    let (completion, next) = SoftwareCpu::new()
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let src = driver.allocate_buffer_on(&mut port, 4).await?;
            let dst = driver.allocate_buffer_on(&mut port, 4).await?;
            driver
                .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
                .await?;
            device.wait_for_progress_after(0).await;
            let completion = driver.poll_completion_on(&mut port).await?;
            let next = driver
                .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
                .await;
            Ok::<_, DriverError>((completion, next))
        })
        .await
        .expect("manual execution remains valid");
    device.request_shutdown().await;
    worker.await.expect("worker joins cleanly");

    // Then: the reclaimed credit is visible in the same CPU poll.
    assert_eq!(
        completion.map(pdma_model_contract::semantic::CompletionRecord::command_id),
        Some(CommandId::new(0))
    );
    assert_eq!(next, Ok(CommandId::new(1)));
}

#[tokio::test(flavor = "current_thread")]
async fn event_pop_reclaims_credit_before_future_can_suspend() {
    // Given: a fault event for a depth-one CPU-bound driver.
    let storage = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new(
        memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let mut port = super::support::BlockingPort::new(device.handle(), memory.clone());

    // When: software pops the event and immediately submits another command.
    let (event, next) = SoftwareCpu::new()
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let dst = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("allocation");
            driver
                .submit_copy_on(&mut port, CopyRequest::new(dst, dst, 5))
                .await?;
            let _snapshot = device.wait_for_error_after(0).await;
            let event = driver.poll_event_on(&mut port).await?;
            let next = driver
                .submit_copy_on(&mut port, CopyRequest::new(dst, dst, 4))
                .await;
            Ok::<_, DriverError>((event, next))
        })
        .await
        .expect("manual execution remains valid");
    device.request_shutdown().await;
    let _summary = worker.await.expect("worker joins cleanly");

    // Then: event handling restores the shared command credit synchronously.
    assert_eq!(
        event.map(pdma_model_contract::semantic::EventRecord::command_id),
        Some(CommandId::new(0))
    );
    assert_eq!(next, Ok(CommandId::new(1)));
}

#[tokio::test(flavor = "current_thread")]
async fn manual_mode_matches_shared_credit_and_reclaim_semantics() {
    // Given: two manual driver capabilities for one depth-one queue.
    let storage = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new(
        memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let mut port = super::support::BlockingPort::new(device.handle(), memory.clone());
    let cpu = SoftwareCpu::new();
    let (mut first, mut second, src, dst, submitted, full) = cpu
        .run_main(async {
            let mut first = ManualDriver::setup_on(&mut port, 1, 1)
                .await
                .expect("manual setup is valid");
            let mut second = first.clone();
            let src = first
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("allocation");
            let dst = first
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("allocation");

            // When: one clone consumes and the peer observes full.
            let submitted = first
                .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
                .await;
            let full = second
                .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
                .await;
            (first, second, src, dst, submitted, full)
        })
        .await;
    device.wait_for_progress_after(0).await;
    let completion = cpu.run_main(second.poll_completion_on(&mut port)).await;
    let retried = cpu
        .run_main(first.submit_copy_on(&mut port, CopyRequest::new(src, dst, 4)))
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker joins cleanly");

    // Then: manual mode uses the same canonical accounting allocation.
    assert_eq!(submitted, Ok(CommandId::new(0)));
    assert_eq!(full, Err(DriverError::QueueFull));
    assert!(matches!(completion, Ok(Some(_))));
    assert_eq!(retried, Ok(CommandId::new(1)));
}
