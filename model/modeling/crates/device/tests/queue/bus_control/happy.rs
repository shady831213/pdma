use super::*;

#[tokio::test]
async fn memory_commit_precedes_queue_activation() -> Result<(), Box<dyn std::error::Error>> {
    // Given: one uncommitted three-ring DMA stage and its exact activation.
    let handle = new_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let proposed = stage.regions().to_vec();
    let request = activation(&stage, QueueId::new(0))?;

    handle.commit_memory_stage(stage).await?;
    assert!(
        handle
            .control_queue(QueueControl::ReadStatus(request.queue()))
            .await
            .is_err()
    );

    // When: activation consumes the separately committed mappings.
    let status = handle.activate_mapped_queue(request).await?;

    // Then: the queue starts empty over the retained mappings.
    assert_eq!(handle.memory_generation().await, 1);
    for region in &proposed {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(*region)
        );
    }
    assert_eq!(
        status.queue(),
        EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0))
    );
    assert_eq!(status.submission().producer(), RingCursor::new(0));
    assert_eq!(status.completion().consumer(), RingCursor::new(0));
    assert_eq!(status.event().consumer(), RingCursor::new(0));
    Ok(())
}

#[tokio::test]
async fn indirect_memory_and_descriptor_precede_producer_publication()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: an active queue, an uncommitted indirect list, and one descriptor slot.
    let handle = new_handle();
    let rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = rings.regions() else {
        return Err("three ring regions required".into());
    };
    let submission_base = submission.base();
    let request = activation(&rings, QueueId::new(0))?;
    handle.commit_memory_stage(rings).await?;
    handle.activate_mapped_queue(request).await?;
    let indirect = handle.stage_memory_regions(&[16]).await?;
    let [indirect_region] = indirect.regions() else {
        return Err("one indirect region required".into());
    };
    let indirect_region = *indirect_region;
    let list_bytes = vec![0x31; 16];
    let descriptor_bytes = vec![0x7a; 64];
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    handle.commit_memory_stage(indirect).await?;
    handle
        .write_memory(indirect_region.base(), &list_bytes)
        .await?;
    handle
        .write_memory(submission_base, &descriptor_bytes)
        .await?;
    assert_eq!(
        handle
            .control_queue(QueueControl::ReadStatus(queue))
            .await?
            .submission()
            .producer(),
        RingCursor::new(0),
    );

    // When: the separate producer publication exposes the descriptor to the queue.
    let status = handle
        .control_queue(QueueControl::PublishSubmissionProducer(
            QueueCursorUpdate::new(queue, RingCursor::new(1)),
        ))
        .await?;

    // Then: earlier memory writes remain and the producer advances once.
    assert_eq!(handle.memory_generation().await, 2);
    assert_eq!(
        handle.region_for_object(indirect_region.object()).await,
        Some(indirect_region)
    );
    assert_eq!(
        handle
            .read_memory(indirect_region.base(), indirect_region.length())
            .await?,
        list_bytes
    );
    assert_eq!(
        handle.read_memory(submission_base, 64).await?,
        descriptor_bytes
    );
    assert_eq!(status.submission().producer(), RingCursor::new(1));
    Ok(())
}

#[tokio::test]
async fn individual_cursor_controls_preserve_idempotent_publication()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: an active depth-two queue with one accepted submission.
    let handle = new_handle();
    let rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = rings.regions() else {
        return Err("three ring regions required".into());
    };
    let submission_base = submission.base();
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    let request = activation(&rings, QueueId::new(0))?;
    handle.commit_memory_stage(rings).await?;
    handle.activate_mapped_queue(request).await?;
    handle.write_memory(submission_base, &[0x44; 64]).await?;
    handle
        .control_queue(QueueControl::PublishSubmissionProducer(
            QueueCursorUpdate::new(queue, RingCursor::new(1)),
        ))
        .await?;

    // When: doorbell, device/result producers, and host result consumers advance.
    for control in [
        QueueControl::CaptureSubmissionProducer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControl::PublishSubmissionConsumer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControl::PublishCompletionProducer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControl::PublishCompletionConsumer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControl::PublishEventProducer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControl::PublishEventConsumer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
    ] {
        handle.control_queue(control).await?;
    }
    let status = handle
        .control_queue(QueueControl::ReadStatus(queue))
        .await?;

    // Then: every typed shadow and doorbell capture reports its committed value.
    assert_eq!(status.captured_submission_producer(), RingCursor::new(1));
    assert_eq!(status.submission().consumer(), RingCursor::new(1));
    assert_eq!(status.completion().producer(), RingCursor::new(1));
    assert_eq!(status.completion().consumer(), RingCursor::new(1));
    assert_eq!(status.event().producer(), RingCursor::new(1));
    assert_eq!(status.event().consumer(), RingCursor::new(1));

    // And when: already-current consumers and the captured producer are repeated.
    let before = handle.snapshot().await;
    for control in [
        QueueControl::CaptureSubmissionProducer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControl::PublishCompletionConsumer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
        QueueControl::PublishEventConsumer(QueueCursorUpdate::new(queue, RingCursor::new(1))),
    ] {
        handle.control_queue(control).await?;
    }
    let after = handle.snapshot().await;

    // Then: idempotent transactions retain status and do not publish a new runner generation.
    assert_eq!(
        handle
            .control_queue(QueueControl::ReadStatus(queue))
            .await?,
        status
    );
    assert_eq!(before.generation(), after.generation());
    Ok(())
}
