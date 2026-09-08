use super::support::{self, TestResult};
use pdma_model_contract::abi::{DmaAddress, mmio::MmioResult};
use pdma_model_contract::semantic::{DeviceEpoch, RingCursor, WholeDeviceReset};
use pdma_model_device::PdmaMemoryError;

#[tokio::test]
async fn malformed_activation_preserves_previously_committed_mapping() -> TestResult {
    // Given: committed rings with a submission extent too short for the requested depth.
    let handle = support::new_handle();
    let stage = handle.stage_memory_regions(&[64, 64, 128]).await?;
    let activation = support::activation(&stage, DeviceEpoch::initial())?;
    let regions = handle.commit_memory_stage(stage).await?;
    support::program_activation(&handle, &regions).await?;

    // When: raw activation validates the actual mapped ring lengths.
    handle.write_mmio(0x168, &1_u32.to_le_bytes()).await?;

    // Then: queue activation fails, but the earlier memory commit remains visible.
    assert_eq!(
        handle.read_mmio(0x16c, 4).await?,
        MmioResult::RingExtentMismatch.code().to_le_bytes()
    );
    assert!(
        support::queue_status(&handle, activation.queue())
            .await
            .is_err()
    );
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
async fn rejected_publication_preserves_written_bytes_and_updates_result_latch() -> TestResult {
    // Given: an active queue and a descriptor write already completed.
    let handle = support::new_handle();
    let (queue, submission, _) = support::bootstrap(&handle).await?;
    handle.write_mmio(0x168, &99_u32.to_le_bytes()).await?;
    handle.write_memory(submission.base(), &[0xaa; 64]).await?;
    let status_before = support::queue_status(&handle, queue).await?;

    // When: producer publication supplies a mismatched expected cursor.
    support::publish(&handle, queue, RingCursor::new(1)).await?;

    // Then: the command rejects without undoing earlier writes or advancing the cursor.
    assert_eq!(
        handle.read_mmio(0x16c, 4).await?,
        MmioResult::InvalidTransaction.code().to_le_bytes()
    );
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        vec![0xaa; 64]
    );
    assert_eq!(support::queue_status(&handle, queue).await?, status_before);
    assert_eq!(handle.read_mmio(0x110, 8).await?, 1_u64.to_le_bytes());
    Ok(())
}

#[tokio::test]
async fn unmapped_descriptor_write_preserves_earlier_indirect_write() -> TestResult {
    // Given: active rings and an independently mapped and written indirect extent.
    let handle = support::new_handle();
    let (queue, submission, _) = support::bootstrap(&handle).await?;
    handle.write_memory(submission.base(), &[0x5a; 64]).await?;
    let indirect = handle.allocate_memory_region(8).await?;
    handle.write_memory(indirect.base(), &[0xbb; 8]).await?;
    let before = handle.snapshot().await;
    let latch_before = support::queue_latches(&handle).await?;

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
    assert_eq!(
        support::queue_status(&handle, queue)
            .await?
            .submission()
            .producer(),
        RingCursor::new(0)
    );
    assert_eq!(support::queue_latches(&handle).await?, latch_before);
    assert_eq!(handle.snapshot().await, before);
    Ok(())
}

#[tokio::test]
async fn stale_activation_after_reset_preserves_committed_memory() -> TestResult {
    // Given: committed ring mappings retained through whole-device reset.
    let handle = support::new_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = support::activation(&stage, DeviceEpoch::initial())?;
    let regions = handle.commit_memory_stage(stage).await?;
    handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    support::program_activation(&handle, &regions).await?;

    // When: activation carries the preceding hardware epoch.
    handle.write_mmio(0x168, &1_u32.to_le_bytes()).await?;

    // Then: stale activation rejects, while reset-persistent mapping state survives.
    assert_eq!(
        handle.read_mmio(0x16c, 4).await?,
        MmioResult::StaleEpoch.code().to_le_bytes()
    );
    assert_eq!(handle.memory_generation().await, 1);
    assert!(
        support::queue_status(&handle, activation.queue())
            .await
            .is_err()
    );
    for region in regions {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(region)
        );
    }
    Ok(())
}
