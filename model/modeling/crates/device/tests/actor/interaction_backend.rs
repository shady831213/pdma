use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{abi::registers::GlobalRegisters, semantic::ResourceConfig};
use pdma_model_device::{AsyncDevice, AsyncStepStatus};

#[tokio::test(flavor = "current_thread")]
async fn interaction_backend_routes_memory_register_reset_and_worker_progress() {
    // Given: a matched actor backend with committed memory.
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
        .expect("write");
    let staged = handle.stage_memory_regions(&[4]).await.expect("stage");
    handle.commit_memory_stage(staged).await.expect("commit");
    let endpoint = handle.register_endpoint();

    // When: a direct reset is followed by an idle worker turn.
    endpoint
        .write(
            u32::try_from(GlobalRegisters::RESET.offset()).expect("reset"),
            &1_u32.to_le_bytes(),
        )
        .await
        .expect("reset");
    let reset = device.snapshot().await;
    let mut epoch = [0; 8];
    endpoint
        .read(
            u32::try_from(GlobalRegisters::EPOCH.offset()).expect("epoch"),
            &mut epoch,
        )
        .await
        .expect("read");
    let worker = tokio::spawn(worker.run());
    let idle = device.wait_for_change_after(reset).await;
    device.request_shutdown().await;
    let summary = worker.await.expect("join");

    // Then: memory persists, epoch advances, and idle work creates no retirement.
    assert_eq!(initial.generation(), 0);
    assert_eq!(u64::from_le_bytes(epoch), 1);
    assert!(reset.generation() > initial.generation());
    assert!(idle.generation() > reset.generation());
    assert_eq!(
        handle.read_memory(region.base(), 8).await.expect("memory"),
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(
        (
            summary.steps,
            summary.errors,
            summary.stalls,
            summary.last_status
        ),
        (0, 0, 0, AsyncStepStatus::Idle)
    );
}
