use super::*;

#[tokio::test]
async fn rejected_and_dropped_publication_preserve_completed_writes() -> TestResult {
    // Given: indirect bytes and a partial descriptor have already committed.
    let handle = new_handle();
    let [submission, _, _] = hardware::activate(&handle, 0, [2; 3]).await?;
    let indirect = handle.allocate_memory_region(8).await?;
    handle.write_memory(indirect.base(), &[4; 8]).await?;
    handle.write_memory(submission, &[3; 63]).await?;
    let endpoint = handle.register_endpoint();
    let offset = queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?;

    // When: one publication rejects and another is dropped without polling.
    assert!(write(&endpoint, offset, 3).await.is_err());
    let producer = 1_u64.to_le_bytes();
    drop(endpoint.write(offset, &producer));

    // Then: DMA bytes persist and the descriptor remains unpublished.
    assert_eq!(handle.memory_generation().await, 2);
    assert_eq!(handle.read_memory(indirect.base(), 8).await?, [4; 8]);
    let mut expected = [3; 64];
    expected[63] = 0;
    assert_eq!(handle.read_memory(submission, 64).await?, expected);
    assert_eq!(read(&endpoint, offset).await?, 0);
    Ok(())
}

#[tokio::test]
async fn stale_allocation_and_absent_queue_leave_committed_rings_intact() -> TestResult {
    // Given: a committed ring stage and a copy of its now-stale allocation proposal.
    let handle = new_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let retained_proposal = stage.clone();
    let regions = handle.commit_memory_stage(stage).await?;
    let endpoint = handle.register_endpoint();

    // When: the stale stage and a skipped queue index are rejected.
    assert!(matches!(
        handle.commit_memory_stage(retained_proposal).await,
        Err(pdma_model_device::PdmaMemoryError::StaleStage {
            expected_generation: 0,
            actual_generation: 1
        })
    ));
    assert!(
        write(
            &endpoint,
            queue_offset(2, QueueRegisters::SQ_BASE.offset())?,
            regions.first().ok_or("first region")?.base().raw()
        )
        .await
        .is_err()
    );

    // Then: the original regions and bytes remain committed without active queues.
    assert_eq!(handle.memory_generation().await, 1);
    for region in regions {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(region)
        );
        assert_eq!(
            handle.read_memory(region.base(), region.length()).await?,
            vec![0; usize::try_from(region.length())?]
        );
    }
    assert_eq!(
        read(
            &endpoint,
            u32::try_from(GlobalRegisters::QUEUE_COUNT.offset())?
        )
        .await?,
        0
    );
    Ok(())
}
