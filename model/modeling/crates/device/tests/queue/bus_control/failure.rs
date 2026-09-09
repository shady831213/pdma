use super::*;

#[tokio::test]
async fn rejected_and_dropped_publication_preserve_completed_writes() -> TestResult {
    // Given: indirect bytes and a partial descriptor have already committed.
    let (handle, memory) = new_handle();
    let [submission, _, _] = hardware::activate(&handle, &memory, 0, [2; 3]).await?;
    let indirect = hardware::allocate(&memory, 8)?;
    handle.write_memory(indirect.base(), &[4; 8]).await?;
    handle.write_memory(submission, &[3; 63]).await?;
    let endpoint = handle.register_endpoint();
    let offset = queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?;

    // When: one publication rejects and another is dropped without polling.
    assert!(write(&endpoint, offset, 3).await.is_err());
    let producer = 1_u64.to_le_bytes();
    drop(endpoint.write(offset, &producer));

    // Then: DMA bytes persist and the descriptor remains unpublished.
    assert_eq!(handle.read_memory(indirect.base(), 8).await?, [4; 8]);
    let mut expected = [3; 64];
    expected[63] = 0;
    assert_eq!(handle.read_memory(submission, 64).await?, expected);
    assert_eq!(read(&endpoint, offset).await?, 0);
    Ok(())
}

#[tokio::test]
async fn absent_queue_leaves_backing_ring_memory_intact() -> TestResult {
    // Given: harness-owned ring bytes exist without active queues.
    let (handle, memory) = new_handle();
    let rings = hardware::rings(&memory, [2; 3])?;
    let endpoint = handle.register_endpoint();

    // When: software addresses a skipped queue index.
    assert!(
        write(
            &endpoint,
            queue_offset(2, QueueRegisters::SQ_BASE.offset())?,
            rings[0].raw()
        )
        .await
        .is_err()
    );

    // Then: backing ring bytes persist without active queues.
    for (base, length) in rings.into_iter().zip([128, 64, 128]) {
        assert_eq!(
            handle.read_memory(base, length).await?,
            vec![0; usize::try_from(length)?]
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
