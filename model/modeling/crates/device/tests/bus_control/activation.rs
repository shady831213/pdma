async fn assert_mapped_activation_rejected(
    handle: &pdma_model_device::AsyncDeviceHandle,
    request: QueueActivation,
    committed: &[DmaRegion],
) -> Result<(), Box<dyn std::error::Error>> {
    let generation = handle.dma_generation().await;
    let run = handle.snapshot().await;
    let trace = handle.trace_events().await;
    let mut region_bytes = Vec::new();
    region_bytes.try_reserve_exact(committed.len())?;
    for region in committed {
        region_bytes.push(handle.read_iova(region.base(), region.length()).await?);
    }

    assert!(handle.activate_mapped_queue(request).await.is_err());
    assert_eq!(handle.dma_generation().await, generation);
    assert_eq!(handle.snapshot().await, run);
    assert_eq!(handle.trace_events().await, trace);
    for (region, before) in committed.iter().zip(&region_bytes) {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(*region)
        );
        assert_eq!(
            handle
                .read_iova(region.base(), region.length())
                .await?
                .as_slice(),
            before.as_slice()
        );
    }
    assert!(
        handle
            .control_queue(QueueControl::ReadStatus(request.queue()))
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn committed_mapped_queue_activation_preserves_dma_and_installs_zeroed_queue()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: three exact, committed SQ/CQ/EQ mappings and their next-queue activation.
    let handle = new_handle();
    let stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let request = activation(&stage, QueueId::new(0))?;
    let committed = handle.commit_dma_stage(stage).await?;
    let generation = handle.dma_generation().await;

    // When: mapped activation consumes the already committed mappings.
    let status = handle.activate_mapped_queue(request).await?;

    // Then: DMA is untouched and one zero-cursor queue with the exact identity is installed.
    assert_eq!(handle.dma_generation().await, generation);
    for region in &committed {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(*region)
        );
        assert_eq!(
            handle.read_iova(region.base(), region.length()).await?,
            vec![0; usize::try_from(region.length())?]
        );
    }
    assert_eq!(
        status.queue(),
        EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0))
    );
    assert_eq!(status.submission().producer(), RingCursor::new(0));
    assert_eq!(status.submission().consumer(), RingCursor::new(0));
    assert_eq!(status.captured_submission_producer(), RingCursor::new(0));
    assert_eq!(status.completion().producer(), RingCursor::new(0));
    assert_eq!(status.completion().consumer(), RingCursor::new(0));
    assert_eq!(status.event().producer(), RingCursor::new(0));
    assert_eq!(status.event().consumer(), RingCursor::new(0));
    Ok(())
}

#[tokio::test]
async fn mapped_activation_rejects_uncommitted_and_partial_mappings_without_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one wholly uncommitted activation and one with only its SQ mapping committed.
    let uncommitted_handle = new_handle();
    let uncommitted = uncommitted_handle
        .stage_dma_regions(&[128, 64, 128])
        .await?;
    let uncommitted_request = activation(&uncommitted, QueueId::new(0))?;
    let partial_handle = new_handle();
    let submission = partial_handle.allocate_dma_region(128).await?;
    let partial = partial_handle.stage_dma_regions(&[64, 128]).await?;
    let [completion, event] = partial.regions() else {
        return Err("two proposed result rings required".into());
    };
    let partial_request = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        pdma_model_contract::QueueConfig::new(
            2,
            2,
            pdma_model_contract::EventRouting::QueueLocal,
            pdma_model_contract::SoftwareOwnerId::new(7),
        ),
        QueueRingRegions::new(
            RingLayout::try_new(submission, 2, RingRecord::Submission)?,
            RingLayout::try_new(*completion, 2, RingRecord::Completion)?,
            RingLayout::try_new(*event, 2, RingRecord::Event)?,
        ),
    )?;

    // When/Then: both mapping sets are rejected before any queue or allocator mutation.
    assert_mapped_activation_rejected(&uncommitted_handle, uncommitted_request, &[]).await?;
    assert_mapped_activation_rejected(&partial_handle, partial_request, &[submission]).await?;
    Ok(())
}

#[tokio::test]
async fn mapped_activation_rejects_wrong_mapping_epoch_depth_role_and_id_without_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: exact committed rings plus validly constructed but stale/wrong mapping requests.
    let handle = new_handle();
    handle.enable_tracing().await;
    let ring_stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let valid = activation(&ring_stage, QueueId::new(0))?;
    let committed = handle.commit_dma_stage(ring_stage).await?;
    let [submission_region, _, _] = committed.as_slice() else {
        return Err("three committed ring regions required".into());
    };
    let submission_region = *submission_region;
    let stale_epoch = QueueActivation::try_new(
        DeviceEpoch::new(1),
        QueueId::new(0),
        valid.config(),
        valid.rings(),
    )?;
    let wrong_id = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(1),
        valid.config(),
        valid.rings(),
    )?;
    let fake_submission = DmaRegion::try_new(
        BufferId::new(99),
        valid.rings().submission().region().base(),
        valid.rings().submission().region().length(),
    )?;
    let wrong_mapping = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        valid.config(),
        QueueRingRegions::new(
            RingLayout::try_new(fake_submission, 2, RingRecord::Submission)?,
            valid.rings().completion(),
            valid.rings().event(),
        ),
    )?;
    let overlapping = QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        valid.config(),
        QueueRingRegions::new(
            valid.rings().submission(),
            valid.rings().completion(),
            RingLayout::try_new(submission_region, 2, RingRecord::Event)?,
        ),
    )?;

    // When/Then: semantic mismatches reject repeatedly with no DMA/queue/trace/wake change.
    for request in [
        stale_epoch,
        wrong_id,
        wrong_mapping,
        overlapping,
        wrong_mapping,
    ] {
        assert_mapped_activation_rejected(&handle, request, &committed).await?;
    }

    // And: wrong depth/role inputs are rejected at their typed construction boundary.
    assert!(
        QueueActivation::try_new(
            DeviceEpoch::initial(),
            QueueId::new(0),
            valid.config(),
            QueueRingRegions::new(
                RingLayout::try_new(submission_region, 4, RingRecord::Completion)?,
                valid.rings().completion(),
                valid.rings().event(),
            ),
        )
        .is_err()
    );
    let misaligned = DmaRegion::try_new(
        submission_region.object(),
        DmaAddress::new(submission_region.base().raw().saturating_add(1)),
        submission_region.length(),
    )?;
    assert!(RingLayout::try_new(misaligned, 2, RingRecord::Submission).is_err());
    let wrong_extent = DmaRegion::try_new(
        submission_region.object(),
        submission_region.base(),
        submission_region.length().saturating_sub(1),
    )?;
    assert!(RingLayout::try_new(wrong_extent, 2, RingRecord::Submission).is_err());
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
    assert_eq!(handle.dma_generation().await, 1);
    assert!(handle.trace_events().await.is_empty());
    Ok(())
}

#[tokio::test]
async fn dropped_mapped_activation_future_changes_nothing_before_commit()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: exact committed rings and an unpolled mapped-activation future.
    let handle = new_handle();
    let stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let request = activation(&stage, QueueId::new(0))?;
    let committed = handle.commit_dma_stage(stage).await?;
    let before = handle.snapshot().await;
    let pending = handle.activate_mapped_queue(request);

    // When: the future is dropped before acquiring the state lock.
    drop(pending);

    // Then: no queue, mapping, generation, trace, or wake state changed.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(handle.dma_generation().await, 1);
    assert!(handle.trace_events().await.is_empty());
    for region in &committed {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(*region)
        );
    }
    assert!(
        handle
            .control_queue(QueueControl::ReadStatus(request.queue()))
            .await
            .is_err()
    );
    Ok(())
}
