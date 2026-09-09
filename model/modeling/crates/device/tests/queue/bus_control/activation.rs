use super::*;

#[tokio::test]
async fn interior_rings_activate_inside_one_backing_extent() -> TestResult {
    let (handle, memory) = new_handle();
    let backing = hardware::allocate(&memory, 512)?;
    let rings = [
        backing.base().checked_add(64)?,
        backing.base().checked_add(128)?,
        backing.base().checked_add(192)?,
    ];
    let endpoint = handle.register_endpoint();
    configure(&endpoint, 0, rings.map(|base| (base, 1))).await?;

    write(
        &endpoint,
        queue_offset(0, QueueRegisters::ENABLE.offset())?,
        1,
    )
    .await?;

    assert_eq!(
        read(&endpoint, queue_offset(0, QueueRegisters::ENABLE.offset())?).await?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn activation_preserves_committed_memory_and_initializes_cursors() -> TestResult {
    // Given: unavailable ring bytes cannot yet be activated.
    let (handle, memory) = new_handle();
    let rings = [0x1000, 0x1080, 0x10c0].map(pdma_model_contract::abi::DmaAddress::new);
    let endpoint = handle.register_endpoint();
    configure(&endpoint, 0, rings.map(|base| (base, 2))).await?;
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            1
        )
        .await
        .is_err()
    );
    // When: the harness supplies the ring bytes and software enables the queue.
    assert_eq!(hardware::rings(&memory, [2; 3])?, rings);
    handle.write_memory(rings[0], &[0x42; 64]).await?;
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::ENABLE.offset())?,
        1,
    )
    .await?;
    // Then: activation preserves mappings and bytes and initializes all cursors.
    assert_eq!(handle.read_memory(rings[0], 64).await?, [0x42; 64]);
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
    Ok(())
}

#[tokio::test]
async fn unavailable_result_rings_reject_without_activation_or_memory_mutation() -> TestResult {
    // Given: only the submission ring has backing storage.
    let (handle, memory) = new_handle();
    let submission = hardware::allocate(&memory, 128)?;
    handle.write_memory(submission.base(), &[0x51; 128]).await?;
    let endpoint = handle.register_endpoint();
    configure(
        &endpoint,
        0,
        [
            (submission.base(), 2),
            (pdma_model_contract::abi::DmaAddress::new(0x1080), 2),
            (pdma_model_contract::abi::DmaAddress::new(0x10c0), 2),
        ],
    )
    .await?;
    let before = handle.snapshot().await;
    // When: software attempts activation against unavailable result rings.
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            1
        )
        .await
        .is_err()
    );
    // Then: rejection preserves memory bytes and inactive queue state.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        handle.read_memory(submission.base(), 128).await?,
        [0x51; 128]
    );
    assert_eq!(
        read(&endpoint, queue_offset(0, QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    Ok(())
}

#[tokio::test]
async fn overlapping_unmapped_and_wrong_extent_configuration_rejects_enable() -> TestResult {
    // Given: three committed exact rings can form one valid queue.
    let (handle, memory) = new_handle();
    let valid = hardware::rings(&memory, [2; 3])?.map(|base| (base, 2));
    let endpoint = handle.register_endpoint();
    // When: software attempts overlapping, absent, and invalid-extent configurations.
    for configuration in [
        [valid[0], valid[1], valid[0]],
        [
            (pdma_model_contract::abi::DmaAddress::new(8), 2),
            valid[1],
            valid[2],
        ],
        [(valid[0].0, 4), valid[1], valid[2]],
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
