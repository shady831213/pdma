use crate::support::raw_hardware;
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, AsyncStallReason, AsyncStepStatus};

#[tokio::test(flavor = "current_thread")]
async fn async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors() {
    // Given: one hardware-visible copy and no read resources.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        ResourceConfig::default().with_read_slots(0),
    );
    let before = runner.snapshot().await;
    let handle = runner.handle();
    let memory = raw_hardware::queue_copy(&handle, &host_memory, &[1, 2, 3, 4]).await;
    let worker = tokio::spawn(worker.run());

    // When: the actor reaches its first terminal resource stall.
    let blocked = runner.wait_for_stall_after(0).await;
    let completion = raw_hardware::completion(&handle, memory.completion).await;
    let destination = handle.read_memory(memory.destination, 4).await;
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
    assert_eq!(completion, None);
    assert_eq!(destination, Ok(vec![0; 4]));
    assert_eq!(summary.stalls, 1);
}
