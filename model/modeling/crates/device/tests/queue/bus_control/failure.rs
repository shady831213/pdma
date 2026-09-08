use super::*;

#[tokio::test]
async fn dropped_and_rejected_activation_preserve_completed_memory_allocations()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: committed rings plus a stale allocation proposal.
    let handle = new_handle();
    let rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let valid = activation(&rings, QueueId::new(0))?;
    let invalid = activation(&rings, QueueId::new(1))?;
    let stale = rings.clone();
    let committed = handle.commit_memory_stage(rings).await?;

    // When: allocation conflicts, wrong queue activation rejects, and a fresh activation drops.
    assert!(matches!(
        handle.commit_memory_stage(stale).await,
        Err(pdma_model_device::PdmaMemoryError::StaleStage {
            expected_generation: 0,
            actual_generation: 1,
        })
    ));
    assert_eq!(
        handle.activate_mapped_queue(invalid).await,
        Err(PdmaControlError::InvalidQueue)
    );
    drop(handle.activate_mapped_queue(valid));

    // Then: memory stays committed while the hardware has no active queue.
    assert_eq!(handle.memory_generation().await, 1);
    for region in committed {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(region)
        );
        assert_eq!(
            handle.read_memory(region.base(), region.length()).await?,
            vec![0; usize::try_from(region.length())?]
        );
    }
    assert!(
        handle
            .control_queue(QueueControl::ReadStatus(valid.queue()))
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn invalid_or_unpolled_publication_preserves_completed_descriptor_and_indirect_writes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: separate indirect and partial descriptor writes on a depth-two queue.
    let handle = new_handle();
    let rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let request = activation(&rings, QueueId::new(0))?;
    let submission = *rings.regions().first().ok_or("submission mapping")?;
    handle.commit_memory_stage(rings).await?;
    handle.activate_mapped_queue(request).await?;
    let indirect = handle.allocate_memory_region(8).await?;
    handle.write_memory(indirect.base(), &[4; 8]).await?;
    handle.write_memory(submission.base(), &[3; 63]).await?;
    let queue = request.queue();

    // When: an out-of-capacity producer rejects and a valid publication is never polled.
    assert!(
        handle
            .control_queue(QueueControl::PublishSubmissionProducer(
                QueueCursorUpdate::new(queue, RingCursor::new(3)),
            ))
            .await
            .is_err()
    );
    drop(
        handle.control_queue(QueueControl::PublishSubmissionProducer(
            QueueCursorUpdate::new(queue, RingCursor::new(1)),
        )),
    );

    // Then: completed bytes and mappings persist without publishing a descriptor.
    assert_eq!(handle.memory_generation().await, 2);
    assert_eq!(
        handle.region_for_object(indirect.object()).await,
        Some(indirect)
    );
    assert_eq!(handle.read_memory(indirect.base(), 8).await?, [4; 8]);
    let mut expected = vec![3; 63];
    expected.push(0);
    assert_eq!(handle.read_memory(submission.base(), 64).await?, expected);
    assert_eq!(
        handle
            .control_queue(QueueControl::ReadStatus(queue))
            .await?
            .submission()
            .producer(),
        RingCursor::new(0),
    );
    Ok(())
}

#[tokio::test]
async fn invalid_epoch_id_layout_overlap_depth_and_cursor_leave_state_unchanged()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: committed rings plus epoch, ID, layout, overlap, and depth-invalid activations.
    let handle = new_handle();
    let rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    let valid = activation(&rings, QueueId::new(0))?;
    let stale = QueueActivation::new(
        DeviceEpoch::new(1),
        QueueId::new(0),
        valid.config(),
        valid.rings(),
    );
    let wrong_id = activation(&rings, QueueId::new(1))?;
    let overlapping = QueueActivation::new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        valid.config(),
        QueueRingObjects::new(
            valid.rings().submission(),
            valid.rings().completion(),
            valid.rings().submission(),
        ),
    );
    let mismatched = QueueActivation::new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        valid.config(),
        QueueRingObjects::new(
            BufferId::new(99),
            valid.rings().completion(),
            valid.rings().event(),
        ),
    );
    let wrong_depth = QueueActivation::new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        pdma_model_contract::semantic::QueueConfig::new(
            1,
            2,
            pdma_model_contract::semantic::EventRouting::QueueLocal,
            pdma_model_contract::semantic::SoftwareOwnerId::new(7),
        ),
        valid.rings(),
    );
    handle.commit_memory_stage(rings).await?;
    for invalid in [stale, wrong_id, overlapping, mismatched, wrong_depth] {
        assert!(handle.activate_mapped_queue(invalid).await.is_err());
        assert_eq!(handle.memory_generation().await, 1);
    }
    handle.activate_mapped_queue(valid).await?;
    let generation = handle.memory_generation().await;

    // When: stale/unknown controls and full, regressing, or beyond-capacity cursors are rejected.
    for control in [
        QueueControl::ReadStatus(EpochQueue::new(DeviceEpoch::new(1), QueueId::new(0))),
        QueueControl::ReadStatus(EpochQueue::new(DeviceEpoch::initial(), QueueId::new(1))),
        QueueControl::PublishCompletionProducer(QueueCursorUpdate::new(queue, RingCursor::new(3))),
        QueueControl::PublishCompletionConsumer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
    ] {
        assert!(handle.control_queue(control).await.is_err());
    }

    // Then: allocator, epoch, identity, mappings, and every cursor remain unchanged.
    assert_eq!(handle.memory_generation().await, generation);
    let status = handle
        .control_queue(QueueControl::ReadStatus(queue))
        .await?;
    assert_eq!(status.queue(), queue);
    assert_eq!(status.submission().producer(), RingCursor::new(0));
    assert_eq!(status.completion().producer(), RingCursor::new(0));
    assert_eq!(status.completion().consumer(), RingCursor::new(0));
    Ok(())
}
