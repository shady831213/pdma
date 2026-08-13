use super::support::{self, TestResult};
use pdma_model_contract::abi::mmio::{MmioByteCount, MmioResult};
use pdma_model_contract::{DeviceEpoch, DmaAddress, QueueControl, RingCursor, WholeDeviceReset};
use pdma_model_device::{DmaWrite, MmioBatchError, QueueBootstrapBatch, SubmissionBatch};

#[tokio::test]
async fn malformed_cardinality_and_exact_write_shapes_commit_nothing() -> TestResult {
    // Given: a canonical activation plus separate wrong-cardinality stages.
    let handle = support::new_handle();
    let canonical = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = support::activation(&canonical, DeviceEpoch::initial())?;
    let [submission_region, _, _] = canonical.regions() else {
        return Err("canonical stage must contain three regions".into());
    };
    let submission_region = *submission_region;
    let short = handle.stage_memory_regions(&[128, 64]).await?;
    let before = handle.snapshot().await;

    // When: bootstrap cardinality/order and submission byte cardinalities are malformed.
    let bootstrap = handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(short, activation))
        .await;
    let descriptor = SubmissionBatch::new(
        activation.queue(),
        RingCursor::new(0),
        DmaWrite::new(submission_region.base(), vec![0; 63]),
        None,
    );
    let submission = handle.submit_queue_batch(descriptor).await;
    let indirect_stage = handle.stage_memory_regions(&[8, 8]).await?;
    let extra_indirect = SubmissionBatch::new(
        activation.queue(),
        RingCursor::new(0),
        DmaWrite::new(submission_region.base(), vec![0; 64]),
        Some((
            indirect_stage,
            DmaWrite::new(DmaAddress::new(0), vec![0; 8]),
        )),
    );
    let indirect = handle.submit_queue_batch(extra_indirect).await;

    // Then: the closed shapes reject before mappings, queue state, or wake state change.
    assert_eq!(bootstrap, Err(MmioBatchError::InvalidShape));
    assert_eq!(submission, Err(MmioBatchError::InvalidShape));
    assert_eq!(indirect, Err(MmioBatchError::InvalidShape));
    assert_eq!(handle.memory_generation().await, 0);
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        handle.region_for_object(submission_region.object()).await,
        None
    );
    Ok(())
}

#[tokio::test]
async fn terminal_command_failure_preserves_dma_cursor_and_standalone_latch() -> TestResult {
    // Given: an active queue with a distinct standalone UNKNOWN_COMMAND result latch.
    let handle = support::new_handle();
    let (queue, submission, _) = support::bootstrap(&handle).await?;
    handle.write_mmio(0x168, &99_u32.to_le_bytes()).await?;
    let latch_before = support::queue_latches(&handle).await?;
    let bytes_before = handle.read_memory(submission.base(), 64).await?;
    let status_before = support::queue_status(&handle, queue).await?;
    let snapshot_before = handle.snapshot().await;

    // When: the terminal publication projects a wrong expected producer.
    let result = handle
        .submit_queue_batch(support::submission(
            queue,
            RingCursor::new(1),
            submission.base(),
            0xaa,
        ))
        .await;

    // Then: terminal non-OK is returned without applying earlier projected writes.
    assert!(
        matches!(result, Err(MmioBatchError::Command(details)) if details.result == MmioResult::InvalidTransaction)
    );
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        bytes_before
    );
    assert_eq!(support::queue_status(&handle, queue).await?, status_before);
    assert_eq!(support::queue_latches(&handle).await?, latch_before);
    assert_eq!(handle.snapshot().await, snapshot_before);
    println!("{{\"terminal_failure\":\"no_mutation\",\"latch\":\"preserved\"}}");
    Ok(())
}

#[tokio::test]
async fn final_dma_operation_failure_discards_prepared_indirect_state() -> TestResult {
    // Given: a valid indirect stage/write followed by an unmapped descriptor address.
    let handle = support::new_handle();
    let (queue, submission, _) = support::bootstrap(&handle).await?;
    handle.write_memory(submission.base(), &[0x5a; 64]).await?;
    let indirect = handle.stage_memory_regions(&[8]).await?;
    let [indirect_region] = indirect.regions() else {
        return Err("one indirect region required".into());
    };
    let indirect_region = *indirect_region;
    let before = handle.snapshot().await;
    let latch_before = support::queue_latches(&handle).await?;

    // When: preparation reaches the final descriptor write and it is unmapped.
    let result = handle
        .submit_queue_batch(SubmissionBatch::new(
            queue,
            RingCursor::new(0),
            DmaWrite::new(DmaAddress::new(0xdead_0000), vec![0xcc; 64]),
            Some((
                indirect,
                DmaWrite::new(indirect_region.base(), vec![0xbb; 8]),
            )),
        ))
        .await;

    // Then: neither prepared mapping/write nor any terminal state becomes visible.
    assert!(
        matches!(result, Err(MmioBatchError::Dma(details)) if details.result == MmioResult::DmaUnmapped && details.details == [0xdead_0000, 64, 0])
    );
    assert_eq!(handle.memory_generation().await, 1);
    assert_eq!(
        handle.region_for_object(indirect_region.object()).await,
        None
    );
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
async fn stale_epoch_terminal_failure_and_unpolled_drop_preserve_state() -> TestResult {
    // Given: an old-epoch bootstrap request and a second request never polled.
    let handle = support::new_handle();
    let stale_stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let stale_activation = support::activation(&stale_stage, DeviceEpoch::initial())?;
    handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    let before = handle.snapshot().await;
    let dropped_stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let dropped_activation = support::activation(&dropped_stage, DeviceEpoch::new(1))?;

    // When: the stale terminal command is awaited and the fresh future drops before polling.
    let stale = handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stale_stage, stale_activation))
        .await;
    let pending =
        handle.bootstrap_queue_batch(QueueBootstrapBatch::new(dropped_stage, dropped_activation));
    drop(pending);

    // Then: both paths leave DMA, queue, latch, and publication state untouched.
    assert!(
        matches!(stale, Err(MmioBatchError::Command(details)) if details.result == MmioResult::StaleEpoch)
    );
    assert_eq!(handle.memory_generation().await, 0);
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        handle.read_mmio(0x16c, MmioByteCount::from(4_u8)).await?,
        0_u32.to_le_bytes()
    );
    assert!(
        handle
            .control_queue(QueueControl::ReadStatus(stale_activation.queue()))
            .await
            .is_err()
    );
    Ok(())
}
