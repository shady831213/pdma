use super::support::{self, TestResult};
use hardware_interface_runtime::{
    MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, RegisterAccess,
};
use pdma_model_contract::abi::registers::{GlobalRegisters, QueueRegisters, queue_offset};
use pdma_model_contract::abi::{AbiControlError, DmaAddress};
use pdma_model_contract::semantic::DeviceEpoch;
use pdma_model_device::{MmioRegisterError, PdmaControlError};

#[tokio::test]
async fn malformed_activation_preserves_previously_committed_mapping() -> TestResult {
    // Given: committed rings with a submission extent too short for the requested depth.
    let handle = support::new_handle();
    let regions = handle.rings([64, 64, 128])?;
    support::program_activation(&handle.register_endpoint(), &regions).await?;

    // When: raw activation validates the actual mapped ring lengths.
    let result = handle
        .register_endpoint()
        .write(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await;

    // Then: queue activation fails, but the earlier memory commit remains visible.
    assert_eq!(
        result,
        Err(MmioRegisterError::Control(PdmaControlError::Memory(
            MemoryAccessError::CrossesRegion {
                address: MemoryAddress::new(regions[0].raw()),
                length: 128,
                region: MemoryRegion::try_new(
                    MemoryAddress::new(regions[0].raw()),
                    64,
                    MemoryAlignment::try_new(64)?
                )?
            }
        )))
    );
    let mut enabled = [0; 4];
    handle
        .register_endpoint()
        .read(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &mut enabled,
        )
        .await?;
    assert_eq!(enabled, 0_u32.to_le_bytes());
    assert_eq!(handle.memory.revision().raw(), 3);
    for (region, length) in regions.into_iter().zip([64, 64, 128]) {
        assert_eq!(
            handle.read_memory(region, length).await?,
            vec![0; usize::try_from(length)?]
        );
    }
    Ok(())
}

#[tokio::test]
async fn rejected_publication_preserves_written_bytes_and_live_cursors() -> TestResult {
    // Given: an active queue and a descriptor write already completed.
    let handle = support::new_handle();
    let submission = support::bootstrap(&handle).await?;
    handle.write_memory(submission, &[0xaa; 64]).await?;
    let status_before = support::cursors(&handle).await?;

    // When: producer publication exceeds the two-entry ring capacity.
    let result = handle
        .register_endpoint()
        .write(
            queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?,
            &3_u64.to_le_bytes(),
        )
        .await;

    // Then: the access rejects without undoing earlier writes or advancing the cursor.
    assert_eq!(
        result,
        Err(MmioRegisterError::Control(PdmaControlError::Abi(
            AbiControlError::CursorDistanceExceeded {
                producer: 3,
                consumer: 0,
                depth: 2
            }
        )))
    );
    assert_eq!(handle.read_memory(submission, 64).await?, vec![0xaa; 64]);
    assert_eq!(support::cursors(&handle).await?, status_before);
    Ok(())
}

#[tokio::test]
async fn unmapped_descriptor_write_preserves_earlier_indirect_write() -> TestResult {
    // Given: active rings and an independently mapped and written indirect extent.
    let handle = support::new_handle();
    let submission = support::bootstrap(&handle).await?;
    handle.write_memory(submission, &[0x5a; 64]).await?;
    let indirect = handle.allocate(8)?;
    handle.write_memory(indirect, &[0xbb; 8]).await?;
    let before = handle.snapshot().await;
    let cursors_before = support::cursors(&handle).await?;

    // When: the next individual write addresses unmapped memory.
    let result = handle
        .write_memory(DmaAddress::new(0xdead_0000), &[0xcc; 64])
        .await;

    // Then: only that access fails; all completed memory operations remain visible.
    assert_eq!(
        result,
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0xdead_0000)
        })
    );
    assert_eq!(handle.memory.revision().raw(), 4);
    assert_eq!(handle.read_memory(indirect, 8).await?, vec![0xbb; 8]);
    assert_eq!(handle.read_memory(submission, 64).await?, vec![0x5a; 64]);
    assert_eq!(support::cursors(&handle).await?, cursors_before);
    assert_eq!(handle.snapshot().await, before);
    Ok(())
}

#[tokio::test]
async fn stale_activation_after_reset_preserves_committed_memory() -> TestResult {
    // Given: committed ring mappings retained through whole-device reset.
    let handle = support::new_handle();
    let regions = handle.rings([128, 64, 128])?;
    let expired = handle.register_endpoint();
    expired
        .write(
            u32::try_from(GlobalRegisters::RESET.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await?;
    let mut epoch = [0; 8];
    expired
        .read(u32::try_from(GlobalRegisters::EPOCH.offset())?, &mut epoch)
        .await?;
    let current = DeviceEpoch::new(u64::from_le_bytes(epoch));
    support::program_activation(&expired.at_epoch(current), &regions).await?;

    // When: activation carries the preceding hardware epoch.
    let result = expired
        .write(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await;

    // Then: stale activation rejects, while reset-persistent mapping state survives.
    assert_eq!(
        result,
        Err(MmioRegisterError::StaleEpoch {
            expected: current,
            actual: DeviceEpoch::initial()
        })
    );
    assert_eq!(handle.memory.revision().raw(), 3);
    let mut enabled = [0; 4];
    expired
        .read(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &mut enabled,
        )
        .await?;
    assert_eq!(enabled, 0_u32.to_le_bytes());
    for (region, length) in regions.into_iter().zip([128, 64, 128]) {
        assert_eq!(
            handle.read_memory(region, length).await?,
            vec![0; usize::try_from(length)?]
        );
    }
    Ok(())
}
