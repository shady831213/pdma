#[tokio::test]
async fn dropped_conflicting_and_invalid_bootstrap_leave_dma_and_queue_unchanged(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given: dropped and conflicting stages plus an activation naming the wrong queue.
    let handle = new_handle();
    let dropped = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let dropped_activation = activation(&dropped, QueueId::new(0))?;
    let pending = handle.activate_queue(dropped, dropped_activation);
    drop(pending);
    let stale = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let winner = handle.stage_memory_regions(&[8]).await?;
    handle.commit_memory_stage(winner).await?;
    let invalid = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let invalid_activation = activation(&invalid, QueueId::new(1))?;

    // When: stale and wrong-ID bootstrap transactions are rejected.
    assert_eq!(
        handle
            .activate_queue(stale.clone(), activation(&stale, QueueId::new(0))?)
            .await,
        Err(BusControlError::InvalidTransaction)
    );
    assert_eq!(
        handle.activate_queue(invalid, invalid_activation).await,
        Err(BusControlError::InvalidQueue)
    );

    // Then: only the independent winner exists and the original ID/base remains reusable.
    assert_eq!(handle.memory_generation().await, 1);
    assert_eq!(handle.region_for_object(BufferId::new(1)).await, None);
    let valid = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [valid_submission, _, _] = valid.regions() else {
        return Err("three valid ring regions required".into());
    };
    assert_eq!(
        *valid_submission,
        invalid_activation.rings().submission().region()
    );
    let status = handle
        .activate_queue(valid.clone(), activation(&valid, QueueId::new(0))?)
        .await?;
    assert_eq!(status.queue().queue_id(), QueueId::new(0));
    Ok(())
}

#[tokio::test]
async fn cancelled_or_invalid_submission_accept_changes_no_dma_bytes_mapping_or_producer(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given: one active queue, a dropped acceptance value, and an invalid descriptor extent.
    let handle = new_handle();
    let rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    let [submission, _, _] = rings.regions() else {
        return Err("three ring regions required".into());
    };
    let submission = *submission;
    handle
        .activate_queue(rings.clone(), activation(&rings, QueueId::new(0))?)
        .await?;
    let indirect = handle.stage_memory_regions(&[8]).await?;
    let [indirect_region] = indirect.regions() else {
        return Err("one indirect region required".into());
    };
    let indirect_region = *indirect_region;
    let cancelled = handle.accept_submission(SubmissionAcceptance::new(
        queue,
        RingCursor::new(0),
        DmaWrite::new(submission.base(), vec![1; 64]),
        Some((
            indirect.clone(),
            DmaWrite::new(indirect_region.base(), vec![2; 8]),
        )),
    ));
    drop(cancelled);
    let invalid = SubmissionAcceptance::new(
        queue,
        RingCursor::new(0),
        DmaWrite::new(submission.base(), vec![3; 63]),
        Some((
            indirect,
            DmaWrite::new(indirect_region.base(), vec![4; 8]),
        )),
    );

    // When: the malformed acceptance is rejected.
    assert!(handle.accept_submission(invalid).await.is_err());

    // Then: no stage, byte, mapping, or producer mutation occurred.
    assert_eq!(handle.memory_generation().await, 1);
    assert_eq!(handle.region_for_object(indirect_region.object()).await, None);
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        vec![0; 64]
    );
    let status = handle.control_queue(QueueControl::ReadStatus(queue)).await?;
    assert_eq!(status.submission().producer(), RingCursor::new(0));
    Ok(())
}

#[tokio::test]
async fn invalid_epoch_id_layout_overlap_depth_and_cursor_leave_state_unchanged(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given: staged rings plus epoch, ID, layout, overlap, and depth-invalid activations.
    let handle = new_handle();
    let rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    let valid = activation(&rings, QueueId::new(0))?;
    let stale = QueueActivation::try_new(
        DeviceEpoch::new(1),
        QueueId::new(0),
        valid.config(),
        valid.rings(),
    )?;
    let wrong_id = activation(&rings, QueueId::new(1))?;
    let overlapping = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        valid.config(),
        QueueRingRegions::new(
            valid.rings().submission(),
            valid.rings().completion(),
            RingLayout::try_new(
                valid.rings().submission().region(),
                2,
                RingRecord::Event,
            )?,
        ),
    )?;
    let fake = PdmaMemoryRegion::try_new(BufferId::new(99), DmaAddress::new(0x2_0000), 128)
        .map_err(|_| "valid fake DMA region required")?;
    let mismatched = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        valid.config(),
        QueueRingRegions::new(
            RingLayout::try_new(fake, 2, RingRecord::Submission)?,
            valid.rings().completion(),
            valid.rings().event(),
        ),
    )?;
    assert!(
        QueueActivation::try_new(
            DeviceEpoch::initial(),
            QueueId::new(0),
            pdma_model_contract::QueueConfig::new(
                1,
                2,
                pdma_model_contract::EventRouting::QueueLocal,
                pdma_model_contract::SoftwareOwnerId::new(7),
            ),
            valid.rings(),
        )
        .is_err()
    );
    for invalid in [stale, wrong_id, overlapping, mismatched] {
        assert!(handle.activate_queue(rings.clone(), invalid).await.is_err());
        assert_eq!(handle.memory_generation().await, 0);
    }
    handle
        .activate_queue(rings.clone(), valid)
        .await?;
    let generation = handle.memory_generation().await;

    // When: stale/unknown controls and full, regressing, or beyond-capacity cursors are rejected.
    for control in [
        QueueControl::ReadStatus(EpochQueue::new(DeviceEpoch::new(1), QueueId::new(0))),
        QueueControl::ReadStatus(EpochQueue::new(DeviceEpoch::initial(), QueueId::new(1))),
        QueueControl::PublishCompletionProducer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(3),
        )),
        QueueControl::PublishCompletionConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
    ] {
        assert!(handle.control_queue(control).await.is_err());
    }

    // Then: allocator, epoch, identity, mappings, and every cursor remain unchanged.
    assert_eq!(handle.memory_generation().await, generation);
    let status = handle.control_queue(QueueControl::ReadStatus(queue)).await?;
    assert_eq!(status.queue(), queue);
    assert_eq!(status.submission().producer(), RingCursor::new(0));
    assert_eq!(status.completion().producer(), RingCursor::new(0));
    assert_eq!(status.completion().consumer(), RingCursor::new(0));
    Ok(())
}
