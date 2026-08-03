#[tokio::test]
async fn mapped_activation_reuses_reset_persistent_mappings_only_at_current_epoch()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: exact committed rings retained while whole-device reset advances the epoch.
    let handle = new_handle();
    let ring_stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let stale_activation = activation(&ring_stage, QueueId::new(0))?;
    let committed = handle.commit_dma_stage(ring_stage).await?;
    let generation = handle.dma_generation().await;
    let current = handle
        .reset(pdma_model_contract::abi::WholeDeviceReset::new(
            DeviceEpoch::initial(),
        ))
        .await?;
    let fresh = QueueActivation::try_new(
        current,
        QueueId::new(0),
        stale_activation.config(),
        stale_activation.rings(),
    )?;

    // When: stale activation rejects and current-epoch activation consumes the same mappings.
    assert!(
        handle
            .activate_mapped_queue(stale_activation)
            .await
            .is_err()
    );
    let status = handle.activate_mapped_queue(fresh).await?;

    // Then: mappings/generation persisted and the new queue carries only the current epoch.
    assert_eq!(handle.dma_generation().await, generation);
    for region in &committed {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(*region)
        );
    }
    assert_eq!(status.queue(), EpochQueue::new(current, QueueId::new(0)));
    assert_eq!(status.submission().producer(), RingCursor::new(0));
    assert_eq!(status.completion().consumer(), RingCursor::new(0));
    assert_eq!(status.event().consumer(), RingCursor::new(0));
    Ok(())
}
