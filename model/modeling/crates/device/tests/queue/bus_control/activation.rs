use super::*;

#[tokio::test]
async fn activation_preserves_committed_memory_and_initializes_cursors() -> TestResult {
    // Given: uncommitted ring mappings cannot yet be activated.
    let handle = new_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let regions = stage.regions().to_vec();
    let [submission, completion, event] = regions.as_slice() else {
        return Err("three rings".into());
    };
    let endpoint = handle.register_endpoint();
    configure(
        &endpoint,
        0,
        [
            (submission.base(), 2),
            (completion.base(), 2),
            (event.base(), 2),
        ],
    )
    .await?;
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            1
        )
        .await
        .is_err()
    );
    // When: software commits the rings and enables the queue.
    handle.commit_memory_stage(stage).await?;
    handle.write_memory(submission.base(), &[0x42; 64]).await?;
    let generation = handle.memory_generation().await;
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::ENABLE.offset())?,
        1,
    )
    .await?;
    // Then: activation preserves mappings and bytes and initializes all cursors.
    assert_eq!(handle.memory_generation().await, generation);
    assert_eq!(handle.read_memory(submission.base(), 64).await?, [0x42; 64]);
    for offset in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::SQ_CAPTURED.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::CQ_CONSUMER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
        QueueRegisters::EQ_CONSUMER.offset(),
    ] {
        assert_eq!(read(&endpoint, queue_offset(0, offset)?).await?, 0);
    }
    for region in regions {
        assert_eq!(
            handle.region_for_object(region.object()).await,
            Some(region)
        );
    }
    Ok(())
}

#[tokio::test]
async fn partial_mapping_rejects_without_activation_or_allocator_mutation() -> TestResult {
    // Given: only the submission mapping is committed.
    let handle = new_handle();
    let submission = handle.allocate_memory_region(128).await?;
    let stage = handle.stage_memory_regions(&[64, 128]).await?;
    let [completion, event] = stage.regions() else {
        return Err("two result rings".into());
    };
    let endpoint = handle.register_endpoint();
    configure(
        &endpoint,
        0,
        [
            (submission.base(), 2),
            (completion.base(), 2),
            (event.base(), 2),
        ],
    )
    .await?;
    let before = handle.snapshot().await;
    let generation = handle.memory_generation().await;
    // When: software attempts activation against uncommitted result rings.
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            1
        )
        .await
        .is_err()
    );
    // Then: rejection preserves allocator and inactive queue state.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(handle.memory_generation().await, generation);
    assert_eq!(
        read(&endpoint, queue_offset(0, QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn overlapping_unmapped_and_wrong_extent_configuration_rejects_enable() -> TestResult {
    // Given: three committed exact rings can form one valid queue.
    let handle = new_handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, completion, event] = stage.regions() else {
        return Err("three rings".into());
    };
    let valid = [
        (submission.base(), 2),
        (completion.base(), 2),
        (event.base(), 2),
    ];
    handle.commit_memory_stage(stage).await?;
    let endpoint = handle.register_endpoint();
    // When: software attempts overlapping, absent, and invalid-extent configurations.
    for configuration in [
        [valid[0], valid[1], valid[0]],
        [
            (pdma_model_contract::abi::DmaAddress::new(8), 2),
            valid[1],
            valid[2],
        ],
        [(valid[0].0, 1), valid[1], valid[2]],
        [(valid[0].0, 0), valid[1], valid[2]],
    ] {
        configure(&endpoint, 0, configuration).await?;
        let before = handle.snapshot().await;
        assert!(
            write(
                &endpoint,
                queue_offset(0, QueueRegisters::ENABLE.offset())?,
                1
            )
            .await
            .is_err()
        );
        assert_eq!(handle.snapshot().await, before);
        assert_eq!(
            read(&endpoint, queue_offset(0, QueueRegisters::ENABLE.offset())?).await?,
            0
        );
    }
    // Then: a subsequent valid activation succeeds and active configuration is immutable.
    configure(&endpoint, 0, valid).await?;
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::ENABLE.offset())?,
        1,
    )
    .await?;
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::SQ_BASE.offset())?,
            valid[0].0.raw()
        )
        .await
        .is_err()
    );
    Ok(())
}
