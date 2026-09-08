use super::support::{self, TestResult};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{GlobalRegisters, QueueRegisters, queue_offset};
use pdma_model_contract::abi::{AbiControlError, DmaAddress};
use pdma_model_contract::semantic::DeviceEpoch;
use pdma_model_device::{MmioRegisterError, PdmaControlError, PdmaMemoryError};

#[tokio::test]
async fn malformed_activation_preserves_previously_committed_mapping() -> TestResult {
    // Given: committed rings with a submission extent too short for the requested depth.
    let handle = support::new_handle();
    let stage = handle.stage_memory_regions(&[64, 64, 128]).await?;
    let regions = handle.commit_memory_stage(stage).await?;
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
        Err(MmioRegisterError::Control(PdmaControlError::Abi(
            AbiControlError::RingExtentMismatch {
                expected: 128,
                actual: 64
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
    assert_eq!(handle.memory_generation().await, 1);
    for region in regions {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(region)
        );
    }
    Ok(())
}

#[tokio::test]
async fn rejected_publication_preserves_written_bytes_and_live_cursors() -> TestResult {
    // Given: an active queue and a descriptor write already completed.
    let handle = support::new_handle();
    let submission = support::bootstrap(&handle).await?;
    handle.write_memory(submission.base(), &[0xaa; 64]).await?;
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
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        vec![0xaa; 64]
    );
    assert_eq!(support::cursors(&handle).await?, status_before);
    Ok(())
}

#[tokio::test]
async fn unmapped_descriptor_write_preserves_earlier_indirect_write() -> TestResult {
    // Given: active rings and an independently mapped and written indirect extent.
    let handle = support::new_handle();
    let submission = support::bootstrap(&handle).await?;
    handle.write_memory(submission.base(), &[0x5a; 64]).await?;
    let indirect = handle.allocate_memory_region(8).await?;
    handle.write_memory(indirect.base(), &[0xbb; 8]).await?;
    let before = handle.snapshot().await;
    let cursors_before = support::cursors(&handle).await?;

    // When: the next individual write addresses unmapped memory.
    let result = handle
        .write_memory(DmaAddress::new(0xdead_0000), &[0xcc; 64])
        .await;

    // Then: only that access fails; all completed memory operations remain visible.
    assert_eq!(result, Err(PdmaMemoryError::Unmapped));
    assert_eq!(handle.memory_generation().await, 2);
    assert_eq!(
        handle.region_for_object(indirect.object()).await,
        Some(indirect)
    );
    assert_eq!(handle.read_memory(indirect.base(), 8).await?, vec![0xbb; 8]);
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        vec![0x5a; 64]
    );
    assert_eq!(support::cursors(&handle).await?, cursors_before);
    assert_eq!(handle.snapshot().await, before);
    Ok(())
}

#[tokio::test]
async fn stale_activation_after_reset_preserves_committed_memory() -> TestResult {
    // Given: committed ring mappings retained through whole-device reset.
    let handle = support::new_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let regions = handle.commit_memory_stage(stage).await?;
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
    assert_eq!(handle.memory_generation().await, 1);
    let mut enabled = [0; 4];
    expired
        .read(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &mut enabled,
        )
        .await?;
    assert_eq!(enabled, 0_u32.to_le_bytes());
    for region in regions {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(region)
        );
    }
    Ok(())
}
