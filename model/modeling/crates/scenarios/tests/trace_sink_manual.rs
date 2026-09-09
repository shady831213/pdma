#![allow(missing_docs)]

use pdma_model_contract::abi::AbiCompletionStatus;
use pdma_model_device::AsyncDevice;

mod support;

#[tokio::test(flavor = "current_thread")]
async fn async_tracing_preserves_manual_run_until_idle_behavior() {
    // Given: the same copy published into separate baseline and traced hardware queues.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (baseline_runner, baseline_worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let baseline_handle = baseline_runner.handle();
    let baseline_memory =
        support::raw_hardware::queue_copy(&baseline_handle, &host_memory, &[4, 3, 2, 1]).await;

    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (traced_runner, traced_worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let traced_handle = traced_runner.handle();
    traced_handle.enable_tracing().await;
    traced_handle.clear_trace().await;
    let traced_memory =
        support::raw_hardware::queue_copy(&traced_handle, &host_memory, &[4, 3, 2, 1]).await;

    // When: both actor workers execute and retire their queued copies.
    let baseline_worker = tokio::spawn(baseline_worker.run());
    let traced_worker = tokio::spawn(traced_worker.run());
    let baseline_summary = baseline_runner.wait_for_progress_after(0).await;
    let traced_summary = traced_runner.wait_for_progress_after(0).await;
    let baseline_completion =
        support::raw_hardware::completion(&baseline_handle, baseline_memory.completion)
            .await
            .expect("baseline completion was published");
    let traced_completion =
        support::raw_hardware::completion(&traced_handle, traced_memory.completion)
            .await
            .expect("traced completion was published");
    let baseline_bytes = baseline_handle
        .read_memory(baseline_memory.destination, 4)
        .await;
    let traced_bytes = traced_handle
        .read_memory(traced_memory.destination, 4)
        .await;
    let traced_events = traced_handle.trace_events().await;
    baseline_runner.request_shutdown().await;
    traced_runner.request_shutdown().await;
    baseline_worker
        .await
        .expect("baseline runner joins cleanly");
    traced_worker.await.expect("traced runner joins cleanly");

    // Then: tracing leaves retirement and copied bytes unchanged while recording events.
    assert_eq!(baseline_summary.steps, 1);
    assert_eq!(traced_summary.steps, 1);
    assert_eq!(baseline_completion.status, AbiCompletionStatus::Success);
    assert_eq!(traced_completion.status, AbiCompletionStatus::Success);
    assert_eq!(baseline_bytes, Ok(vec![4, 3, 2, 1]));
    assert_eq!(traced_bytes, Ok(vec![4, 3, 2, 1]));
    assert!(!traced_events.is_empty());
}
