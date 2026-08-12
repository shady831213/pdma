use super::support::{self, TestResult};
use pdma_model_contract::abi::RingCursor;
use pdma_model_device::{DmaWrite, SubmissionBatch};

#[tokio::test]
async fn closed_batches_commit_exact_order_and_codec() -> TestResult {
    // Given: one fresh device and the exact three-ring bootstrap grammar.
    let handle = support::new_handle();

    // When: bootstrap then a descriptor-only submission commit in order.
    let (queue, submission, bootstrap_bytes) = support::bootstrap(&handle).await?;
    let bootstrap_status = support::queue_status(&handle, queue).await?;
    let submission_bytes = handle
        .submit_queue_batch(support::submission(
            queue,
            RingCursor::new(0),
            submission.base(),
            0x6b,
        ))
        .await?;
    let submission_status = support::queue_status(&handle, queue).await?;

    // Then: both fixed reads equal the canonical 72-byte little-endian codec.
    assert_eq!(
        bootstrap_bytes,
        support::response(bootstrap_status)
            .to_le_bytes()
            .expect("generated queue response view must encode")
    );
    assert_eq!(
        submission_bytes,
        support::response(submission_status)
            .to_le_bytes()
            .expect("generated queue response view must encode")
    );
    assert_eq!(
        submission_status.submission().producer(),
        RingCursor::new(1)
    );
    assert_eq!(
        handle.read_iova(submission.base(), 64).await?,
        vec![0x6b; 64]
    );
    assert_eq!(handle.dma_generation().await, 1);
    assert_eq!(handle.snapshot().await.generation(), 2);
    println!(
        "{{\"operations\":[\"stage3\",\"activate\",\"read72\",\"write64\",\"publish_sq\",\"read72\"]}}"
    );
    Ok(())
}

#[tokio::test]
async fn submission_batch_commits_one_exact_indirect_region_before_descriptor() -> TestResult {
    // Given: an active queue and one exact indirect allocation proposal.
    let handle = support::new_handle();
    let (queue, submission, _) = support::bootstrap(&handle).await?;
    let indirect = handle.stage_dma_regions(&[16]).await?;
    let [region] = indirect.regions() else {
        return Err("one indirect region required".into());
    };
    let region = *region;

    // When: the optional stage/write precedes the descriptor and publication.
    let response = handle
        .submit_queue_batch(SubmissionBatch::new(
            queue,
            RingCursor::new(0),
            DmaWrite::new(submission.base(), vec![0x44; 64]),
            Some((indirect, DmaWrite::new(region.base(), vec![0x33; 16]))),
        ))
        .await?;

    // Then: the entire ordered transaction is visible together.
    let status = support::queue_status(&handle, queue).await?;
    assert_eq!(
        response,
        support::response(status)
            .to_le_bytes()
            .expect("generated queue response view must encode")
    );
    assert_eq!(handle.read_iova(region.base(), 16).await?, vec![0x33; 16]);
    assert_eq!(
        handle.read_iova(submission.base(), 64).await?,
        vec![0x44; 64]
    );
    assert_eq!(handle.dma_generation().await, 2);
    Ok(())
}
