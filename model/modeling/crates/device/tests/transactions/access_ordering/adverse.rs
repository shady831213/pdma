use super::*;
use pdma_model_device::PdmaMemoryError;

pub(super) mod writer;

pub(super) async fn run() -> support::TestResult {
    // Given: an obsolete memory proposal and an active observed queue.
    let (handle, _) = support::observed_handle();
    let stale = handle.stage_memory_regions(&[128, 64, 128]).await?;
    handle.allocate_memory_region(1).await?;
    let generation = handle.memory_generation().await;
    let (active, observer) = support::observed_handle();
    let submission = support::bootstrap(&active).await?;
    let status = support::cursors(&active).await?;
    let before = active.snapshot().await;
    let observations = observer.snapshot().len();
    let endpoint = active.register_endpoint();

    // When: stale mapping and malformed individual register/memory accesses are attempted.
    assert_eq!(
        handle.commit_memory_stage(stale).await,
        Err(PdmaMemoryError::StaleStage {
            expected_generation: 0,
            actual_generation: 1
        })
    );
    assert!(endpoint.read(0, &mut [0; 1]).await.is_err());
    assert!(endpoint.write(0, &[0; 4]).await.is_err());
    assert!(
        endpoint
            .write(
                u32::try_from(GlobalRegisters::RESET.offset())?,
                &u32::MAX.to_le_bytes()
            )
            .await
            .is_err()
    );
    assert_eq!(
        active.write_memory(submission.base(), &[0; 129]).await,
        Err(PdmaMemoryError::CrossesRegion)
    );

    // Then: each rejected access preserves its hardware data and emits no register observation.
    assert_eq!(handle.memory_generation().await, generation);
    assert_eq!(active.snapshot().await, before);
    assert_eq!(observer.snapshot().len(), observations);
    assert_eq!(support::cursors(&active).await?, status);
    assert_eq!(
        active.read_memory(submission.base(), 128).await?,
        vec![0; 128]
    );
    capacity_and_overlap().await?;
    Ok(())
}

async fn capacity_and_overlap() -> support::TestResult {
    // Given: bounded storage and an independently installed overlapping runtime region.
    let capacity_memory =
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::with_region_capacity(2));
    let (capacity_handle, capacity_observer) =
        support::observed_handle_for_memory(&capacity_memory);
    let capacity_stage = capacity_handle
        .stage_memory_regions(&[128, 64, 128])
        .await?;
    let overlap_memory = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let overlapping = MemoryRegion::try_new(
        MemoryAddress::new(0x1_0000),
        64,
        MemoryAlignment::try_new(64)?,
    )?;
    overlap_memory
        .prepare_regions(&[overlapping])?
        .try_install(&overlap_memory)?;
    let revision = overlap_memory.revision();
    let (overlap_handle, overlap_observer) = support::observed_handle_for_memory(&overlap_memory);
    let overlap_stage = overlap_handle.stage_memory_regions(&[128, 64, 128]).await?;

    // When: standalone memory commits encounter storage capacity and overlap.
    assert_eq!(
        capacity_handle.commit_memory_stage(capacity_stage).await,
        Err(PdmaMemoryError::StorageExhausted)
    );
    assert_eq!(
        overlap_handle.commit_memory_stage(overlap_stage).await,
        Err(PdmaMemoryError::OverlappingRegion)
    );

    // Then: neither failed memory-only commit changes mappings or emits MMIO observations.
    assert!(capacity_memory.regions().is_empty());
    assert_eq!(capacity_handle.memory_generation().await, 0);
    assert!(capacity_observer.snapshot().is_empty());
    assert_eq!(overlap_memory.revision(), revision);
    assert_eq!(overlap_memory.regions(), vec![overlapping]);
    assert_eq!(overlap_handle.memory_generation().await, 0);
    assert!(overlap_observer.snapshot().is_empty());
    Ok(())
}
