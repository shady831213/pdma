use super::support::{RecordingObserver, TestResult, create_message, submit_copy};
use hardware_interface_runtime::{AccessOrigin, RegisterAccess};
use hardware_interface_schema::register::Access;
use pdma_model_contract::abi::registers::PdmaRegisterMap;
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::AsyncDevice;
use std::sync::Arc;

pub(super) async fn run() -> TestResult {
    // Given: an observed endpoint, generated direct register placements and a real worker.
    let observer = Arc::new(RecordingObserver::default());
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        Arc::new(hardware_interface_runtime::SharedMemory::new()),
        hardware_interface_runtime::MemoryAddress::new(0x1000),
    );
    let (device, worker) = AsyncDevice::new_observed(
        allocator.memory_access(),
        ResourceConfig::default(),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let resolved = PdmaRegisterMap::resolve()?;
    let submission = create_message(&handle, &allocator).await?;

    // When: every readable placement and a real message-producing copy use the hardware.
    let mut reads = 0;
    for register in resolved.registers() {
        let definition = register.definition();
        if definition.access() != Access::WriteOnly {
            endpoint
                .read(
                    u32::try_from(register.offset())?,
                    &mut vec![0_u8; definition.width()],
                )
                .await?;
            reads += 1;
        }
    }
    submit_copy(&handle, &allocator, submission).await?;
    device.wait_for_progress_after(0).await;
    let before_claim = observer.snapshot();
    let notification = super::support::claim(&handle).await?;
    let observations = observer.snapshot();

    // Then: successful bus accesses have ordered observations; actual IRQ output is separate.
    assert_eq!(
        reads,
        resolved
            .registers()
            .iter()
            .filter(|register| register.definition().access() != Access::WriteOnly)
            .count()
    );
    assert!(notification);
    assert_eq!(observations, before_claim);
    assert!(
        observations
            .iter()
            .all(|item| item.origin == AccessOrigin::Direct)
    );
    assert!(
        observations
            .iter()
            .enumerate()
            .all(|(index, item)| usize::try_from(item.sequence) == Ok(index))
    );
    println!(
        "direct_register_reads={reads};ordered_observations={};irq_output=message",
        observations.len()
    );
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
