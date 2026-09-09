use super::*;
use hardware_interface_runtime::{MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::DmaAddress;

#[tokio::test]
async fn skipped_queue_identity_rejects_without_allocating_a_slot() -> TestResult {
    // Given: an untouched hardware queue collection.
    let (device, _worker, allocator) = device(ResourceConfig::default());
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
    create_queue(&handle, &allocator, 2).await?;
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}

#[tokio::test]
async fn raw_activation_rejects_unavailable_backing_without_mutation() -> TestResult {
    // Given: configured ring addresses with no backing bytes.
    let (device, _worker, allocator) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let rings = [
        DmaAddress::new(0x1000),
        DmaAddress::new(0x1080),
        DmaAddress::new(0x10c0),
    ];
    program_activation(&endpoint, &rings, 2).await?;
    let before = handle.snapshot().await;
    // When: enabling repeatedly encounters an unavailable SQ extent.
    for _ in 0..2 {
        assert!(matches!(
            write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
            Err(MmioRegisterError::Control(PdmaControlError::Memory(
                MemoryAccessError::Unmapped { address }
            ))) if address == MemoryAddress::new(0x1000)
        ));
    }
    // Then: rejection preserves state; supplying backing permits the same configuration.
    assert_eq!(handle.snapshot().await, before);
    for length in [128, 64, 128] {
        allocator.allocate(length, MemoryAlignment::try_new(64)?)?;
    }
    write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await?;
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}

#[tokio::test]
async fn partial_backing_rejection_preserves_inactive_queue() -> TestResult {
    // Given: backing only for SQ, with unavailable CQ and EQ addresses.
    let (device, _worker, allocator) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let submission = DmaAddress::new(
        allocator
            .allocate(128, MemoryAlignment::try_new(64)?)?
            .raw(),
    );
    program_activation(
        &endpoint,
        &[submission, DmaAddress::new(0x1080), DmaAddress::new(0x10c0)],
        2,
    )
    .await?;
    let before = handle.snapshot().await;
    // When: enabling reaches the unavailable CQ extent.
    assert!(matches!(
        write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
        Err(MmioRegisterError::Control(PdmaControlError::Memory(
            MemoryAccessError::Unmapped { address }
        ))) if address == MemoryAddress::new(0x1080)
    ));
    // Then: no partial activation or memory mutation occurs.
    assert_eq!(
        read_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    assert_eq!(handle.snapshot().await, before);
    Ok(())
}

#[tokio::test]
async fn short_extent_and_misaligned_base_reject_before_activation() -> TestResult {
    // Given: a backed SQ shorter than its configured depth-two extent.
    let (device, _worker, allocator) = device(ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let alignment = MemoryAlignment::try_new(64)?;
    let [submission, completion, event] =
        [127, 64, 128].map(|length| allocator.allocate(length, alignment));
    let regions = [submission?, completion?, event?].map(|address| DmaAddress::new(address.raw()));
    program_activation(&endpoint, &regions, 2).await?;
    // When: activation sees a wrong extent, then a misaligned SQ address.
    assert!(matches!(
        write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
        Err(MmioRegisterError::Control(PdmaControlError::Memory(
            MemoryAccessError::CrossesRegion { length: 128, region, .. }
        ))) if region.length() == 127
    ));
    write_u64(
        &endpoint,
        q(QueueRegisters::SQ_BASE.offset())?,
        regions.first().ok_or("submission region")?.raw() + 1,
    )
    .await?;
    assert_eq!(
        write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await,
        Err(MmioRegisterError::Control(PdmaControlError::Abi(
            AbiControlError::MisalignedRing {
                base: regions.first().ok_or("submission region")?.raw() + 1,
                alignment: 64,
            }
        )))
    );
    // Then: both invalid configurations leave all cursor state inactive.
    assert_eq!(
        read_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}

#[tokio::test]
async fn aligned_interior_ring_extents_activate_without_device_registration() -> TestResult {
    // Given: one backend allocation containing all three aligned ring extents.
    let (device, _worker, allocator) = device(ResourceConfig::default());
    let base = allocator.allocate(512, MemoryAlignment::try_new(64)?)?;
    let rings = [64, 192, 256].map(|offset| DmaAddress::new(base.raw() + offset));
    let endpoint = device.handle().register_endpoint();
    program_activation(&endpoint, &rings, 2).await?;
    // When: raw MMIO enables rings whose bases are interior backend addresses.
    write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await?;
    // Then: the queue is active with empty cursors.
    assert_eq!(
        read_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?).await?,
        1
    );
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}
