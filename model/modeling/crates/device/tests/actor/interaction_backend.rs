#![allow(missing_docs)]

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    abi::mmio::MmioByteCount,
    semantic::{EventRouting, QueueConfig, ResourceConfig, SoftwareOwnerId, WholeDeviceReset},
};
use pdma_model_device::{AsyncDevice, AsyncStepStatus};

#[tokio::test(flavor = "current_thread")]
async fn interaction_backend_routes_complete_actor_surface() {
    let (device, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let initial = device.snapshot().await;
    let region = handle.allocate_memory_region(8).await.expect("allocation");
    handle
        .write_memory(region.base(), &[1, 2, 3, 4, 5, 6, 7, 8])
        .await
        .expect("mapped write");
    let staged = handle.stage_memory_regions(&[4]).await.expect("stage");
    handle
        .commit_memory_stage(staged)
        .await
        .expect("commit stage");
    let queue = handle
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(9),
        ))
        .await
        .expect("queue");
    handle
        .enable_irq_source(queue.completion_irq_source())
        .await
        .expect("IRQ source");
    let status = handle.current_status().await;
    let raw_status = handle
        .read_mmio(0x28, MmioByteCount::from(8_u8))
        .await
        .expect("raw status");
    let mut endpoint_status = [0_u8; 8];
    handle
        .register_endpoint()
        .read(0x28, &mut endpoint_status)
        .await
        .expect("endpoint status");
    let epoch = handle
        .reset(WholeDeviceReset::new(status.epoch()))
        .await
        .expect("reset");
    let reset = device.snapshot().await;

    assert_eq!(initial.generation(), 0);
    assert_eq!(
        handle.read_memory(region.base(), 8).await,
        Ok(vec![1, 2, 3, 4, 5, 6, 7, 8])
    );
    assert_eq!(raw_status, endpoint_status);
    assert_eq!(epoch.raw(), 1);
    assert_eq!(
        (
            reset.steps,
            reset.errors,
            reset.stalls,
            reset.last_status,
            reset.shutdown_requested,
            reset.generation(),
        ),
        (0, 0, 0, AsyncStepStatus::Idle, false, 6)
    );

    let worker = tokio::spawn(worker.run());
    let idle = device.wait_for_change_after(reset).await;
    assert_eq!(idle.generation(), 7);
    device.request_shutdown().await;
    let summary = worker.await.expect("worker joins");
    assert_eq!(
        (
            summary.steps,
            summary.errors,
            summary.stalls,
            summary.last_status
        ),
        (0, 0, 0, AsyncStepStatus::Idle)
    );
    println!(
        "closures=actor_surface memory={:?} mappings=2 queue=0 irq_ready=false reset_epoch={} generations=0/6/7 observations=1 worker=steps:{} stalls:{} errors:{}",
        handle.read_memory(region.base(), 8).await.expect("read"),
        epoch.raw(),
        summary.steps,
        summary.stalls,
        summary.errors,
    );
}
