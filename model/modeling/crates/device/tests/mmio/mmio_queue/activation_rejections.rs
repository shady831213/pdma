use super::*;
use pdma_model_device::PdmaMemoryError;

#[tokio::test]
async fn skipped_queue_identity_rejects_without_allocating_a_slot() -> TestResult {
    // Given: an untouched hardware queue collection.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let before = handle.snapshot().await;
    // When: configuration skips the next available queue identity.
    let offset =
        pdma_model_contract::abi::registers::queue_offset(1, QueueRegisters::SQ_BASE.offset())?;
    assert_eq!(
        write_u64(&endpoint, offset, 0x1000).await,
        Err(MmioRegisterError::Control(PdmaControlError::InvalidQueue))
    );
    // Then: no slot is allocated and queue zero remains available for exact activation.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        read_u32(
            &endpoint,
            u32::try_from(GlobalRegisters::QUEUE_COUNT.offset())?
        )
        .await?,
        0
    );
    create_queue(&handle, 2).await?;
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}

#[tokio::test]
async fn raw_activation_rejects_uncommitted_partial_and_wrong_extent_mappings_without_mutation()
-> TestResult {
    // Given: mappings staged but not committed to hardware.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    program_activation(&endpoint, stage.regions(), 2).await?;
    let before = handle.snapshot().await;
    // When: enabling repeatedly encounters uncommitted DMA.
    for _ in 0..2 {
        assert!(matches!(
            write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
            Err(MmioRegisterError::Control(PdmaControlError::Dma(
                PdmaMemoryError::InvalidRegion
            )))
        ));
    }
    // Then: rejection preserves state, and committing the same mappings permits activation.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(handle.memory_generation().await, 0);
    handle.commit_memory_stage(stage).await?;
    write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await?;
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}

#[tokio::test]
async fn partial_mapping_rejection_preserves_inactive_queue() -> TestResult {
    // Given: a committed SQ and uncommitted CQ/EQ.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let submission = handle.allocate_memory_region(128).await?;
    let stage = handle.stage_memory_regions(&[64, 128]).await?;
    let [completion, event] = stage.regions() else {
        return Err("two regions".into());
    };
    program_activation(&endpoint, &[submission, *completion, *event], 2).await?;
    let generation = handle.memory_generation().await;
    // When: enabling encounters incomplete committed mappings.
    assert!(matches!(
        write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
        Err(MmioRegisterError::Control(PdmaControlError::Dma(
            PdmaMemoryError::InvalidRegion
        )))
    ));
    // Then: no partial activation or memory mutation occurs.
    assert_eq!(
        read_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    assert_eq!(handle.memory_generation().await, generation);
    Ok(())
}

#[tokio::test]
async fn wrong_extent_and_misaligned_base_reject_before_activation() -> TestResult {
    // Given: a committed SQ shorter than its configured depth-two extent.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let stage = handle.stage_memory_regions(&[127, 64, 128]).await?;
    let regions = handle.commit_memory_stage(stage).await?;
    program_activation(&endpoint, &regions, 2).await?;
    // When: activation sees a wrong extent, then a misaligned SQ address.
    assert!(matches!(
        write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
        Err(MmioRegisterError::Control(PdmaControlError::Abi(
            AbiControlError::RingExtentMismatch {
                expected: 128,
                actual: 127
            }
        )))
    ));
    write_u64(
        &endpoint,
        q(QueueRegisters::SQ_BASE.offset())?,
        regions.first().ok_or("submission region")?.base().raw() + 1,
    )
    .await?;
    assert!(matches!(
        write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
        Err(MmioRegisterError::Control(PdmaControlError::Dma(
            PdmaMemoryError::InvalidRegion
        )))
    ));
    // Then: both invalid configurations leave all cursor state inactive.
    assert_eq!(
        read_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}
