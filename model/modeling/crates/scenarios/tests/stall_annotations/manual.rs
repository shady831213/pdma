use pdma_model_contract::semantic::{ResourceConfig, desc::CopyRequest};
use pdma_model_device::{AsyncDevice, AsyncStallReason, AsyncStepStatus};
use pdma_model_host::{ManualDriver, compat::AsyncDevicePort};

#[tokio::test(flavor = "current_thread")]
async fn async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors() {
    // Given: a CPU-unbound observability driver with one copy and no read resources.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_read_slots(0),
    ));
    let before = runner.snapshot().await;
    let mut port = AsyncDevicePort::new(runner.handle());
    let mut driver = ManualDriver::setup_on(&mut port, 4, 4)
        .await
        .expect("queue setup is valid");
    let src = driver
        .allocate_buffer_on(&mut port, 4)
        .await
        .expect("allocation");
    let dst = driver
        .allocate_buffer_on(&mut port, 4)
        .await
        .expect("allocation");
    driver
        .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
        .await
        .expect("copy submission has ring space");
    let worker = tokio::spawn(worker.run());

    // When: the actor reaches its first terminal resource stall.
    let blocked = runner.wait_for_stall_after(0).await;
    let completion = driver.poll_completion_on(&mut port).await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the stall is observable without fabricating progress or an idle-path error.
    assert_eq!(before.last_status, AsyncStepStatus::Idle);
    assert_eq!(before.errors, 0);
    assert_eq!(
        blocked.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::ExecutionResourcePressure)
    );
    assert_eq!(blocked.steps, 0);
    assert_eq!(blocked.errors, 0);
    assert_eq!(completion, Ok(None));
    assert_eq!(summary.stalls, 1);
}
