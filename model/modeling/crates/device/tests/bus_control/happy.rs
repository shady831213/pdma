#[tokio::test]
async fn staged_ring_batch_and_queue_activation_commit_atomically(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given: one uncommitted three-ring DMA stage and its exact activation.
    let handle = new_handle();
    let stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let proposed = stage.regions().to_vec();
    let request = activation(&stage, QueueId::new(0))?;

    // When: the bootstrap transaction commits.
    let status = handle.activate_queue(stage, request).await?;

    // Then: all mappings, allocator generation, identity, and zeroed cursors appear together.
    assert_eq!(handle.dma_generation().await, 1);
    for region in &proposed {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(*region)
        );
    }
    assert_eq!(status.queue(), EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0)));
    assert_eq!(status.submission().producer(), RingCursor::new(0));
    assert_eq!(status.completion().consumer(), RingCursor::new(0));
    assert_eq!(status.event().consumer(), RingCursor::new(0));
    Ok(())
}

#[tokio::test]
async fn staged_indirect_list_descriptor_and_producer_commit_atomically(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given: an active queue, an uncommitted indirect list, and one descriptor slot.
    let handle = new_handle();
    let rings = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = rings.regions() else {
        return Err("three ring regions required".into());
    };
    let submission_base = submission.base();
    handle
        .activate_queue(rings.clone(), activation(&rings, QueueId::new(0))?)
        .await?;
    let indirect = handle.stage_dma_regions(&[16]).await?;
    let [indirect_region] = indirect.regions() else {
        return Err("one indirect region required".into());
    };
    let indirect_region = *indirect_region;
    let list_bytes = vec![0x31; 16];
    let descriptor_bytes = vec![0x7a; 64];
    let accept = SubmissionAcceptance::new(
        EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0)),
        RingCursor::new(0),
        DmaWrite::new(submission_base, descriptor_bytes.clone()),
        Some((indirect, DmaWrite::new(indirect_region.base(), list_bytes.clone()))),
    );

    // When: submission acceptance commits.
    let status = handle.accept_submission(accept).await?;

    // Then: the optional mapping, both byte writes, and producer are visible together.
    assert_eq!(handle.dma_generation().await, 2);
    assert_eq!(
        handle.region_for_object(indirect_region.object()).await,
        Some(indirect_region)
    );
    assert_eq!(
        handle
            .read_iova(indirect_region.base(), indirect_region.length())
            .await?,
        list_bytes
    );
    assert_eq!(
        handle.read_iova(submission_base, 64).await?,
        descriptor_bytes
    );
    assert_eq!(status.submission().producer(), RingCursor::new(1));
    Ok(())
}

#[tokio::test]
async fn queue_activation_and_cursor_transactions_commit_atomically(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given: an active depth-two queue with one accepted submission.
    let handle = new_handle();
    let rings = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = rings.regions() else {
        return Err("three ring regions required".into());
    };
    let descriptor = DmaWrite::new(submission.base(), vec![0x44; 64]);
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    handle
        .activate_queue(rings.clone(), activation(&rings, QueueId::new(0))?)
        .await?;
    handle
        .accept_submission(SubmissionAcceptance::new(
            queue,
            RingCursor::new(0),
            descriptor,
            None,
        ))
        .await?;

    // When: doorbell, device/result producers, and host result consumers advance.
    for control in [
        QueueControl::CaptureSubmissionProducer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControl::PublishSubmissionConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControl::PublishCompletionProducer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControl::PublishCompletionConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControl::PublishEventProducer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControl::PublishEventConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
    ] {
        handle.control_queue(control).await?;
    }
    let status = handle.control_queue(QueueControl::ReadStatus(queue)).await?;

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
        QueueControl::CaptureSubmissionProducer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControl::PublishCompletionConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
        QueueControl::PublishEventConsumer(QueueCursorUpdate::new(
            queue,
            RingCursor::new(1),
        )),
    ] {
        handle.control_queue(control).await?;
    }
    let after = handle.snapshot().await;

    // Then: idempotent transactions retain status and do not publish a new runner generation.
    assert_eq!(
        handle.control_queue(QueueControl::ReadStatus(queue)).await?,
        status
    );
    assert_eq!(before.generation(), after.generation());
    Ok(())
}
