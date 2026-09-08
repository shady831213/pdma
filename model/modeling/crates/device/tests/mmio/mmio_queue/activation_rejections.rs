use super::*;

#[tokio::test]
async fn raw_activation_rejects_uncommitted_partial_and_wrong_extent_mappings_without_mutation()
-> TestResult {
    // Given: uncommitted, partially committed, and wrong-extent mapping sets.
    let (uncommitted_device, _worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let uncommitted = uncommitted_device.handle();
    let proposed = uncommitted.stage_memory_regions(&[128, 64, 128]).await?;
    let [sq, cq, eq] = proposed.regions() else {
        return Err("three proposed regions required".into());
    };
    program_activation(
        &uncommitted,
        RawActivation::depth_two(DeviceEpoch::initial(), QueueId::new(0), [*sq, *cq, *eq]),
    )
    .await?;

    // When: repeated ACTIVATE commands target DMA that has not been committed.
    for _ in 0..2 {
        issue_queue_command(&uncommitted, 1).await?;
        assert_eq!(
            read_u32(&uncommitted, 0x168).await?,
            MmioResult::DmaUnmapped.code()
        );
    }

    // Then: the allocator and queue identity remain reusable by one exact mapped activation.
    assert_eq!(uncommitted.memory_generation().await, 0);
    let committed = uncommitted.commit_memory_stage(proposed).await?;
    activate_raw(
        &uncommitted,
        DeviceEpoch::initial(),
        QueueId::new(0),
        &committed,
    )
    .await?;
    assert_eq!(read_u32(&uncommitted, 0x168).await?, MmioResult::Ok.code());

    let (partial_device, _worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let partial = partial_device.handle();
    let submission_mapping = partial.allocate_memory_region(128).await?;
    let pending = partial.stage_memory_regions(&[64, 128]).await?;
    let [completion_proposal, event_proposal] = pending.regions() else {
        return Err("two proposed result regions required".into());
    };
    program_activation(
        &partial,
        RawActivation::depth_two(
            DeviceEpoch::initial(),
            QueueId::new(0),
            [submission_mapping, *completion_proposal, *event_proposal],
        ),
    )
    .await?;
    issue_queue_command(&partial, 1).await?;
    assert_eq!(
        read_u32(&partial, 0x168).await?,
        MmioResult::DmaUnmapped.code()
    );

    let (extent_device, _worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let extent = extent_device.handle();
    let wrong_extent = extent.stage_memory_regions(&[127, 64, 128]).await?;
    let wrong_extent = extent.commit_memory_stage(wrong_extent).await?;
    activate_raw(
        &extent,
        DeviceEpoch::initial(),
        QueueId::new(0),
        &wrong_extent,
    )
    .await?;
    assert_eq!(
        read_u32(&extent, 0x168).await?,
        MmioResult::RingExtentMismatch.code()
    );
    Ok(())
}
