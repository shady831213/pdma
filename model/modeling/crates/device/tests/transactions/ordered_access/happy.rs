use super::support::{self, TestResult};
use hardware_interface_runtime::AccessOrigin;
use pdma_model_contract::semantic::RingCursor;

#[tokio::test]
async fn individual_accesses_publish_exact_queue_response() -> TestResult {
    // Given: three independently committed rings and a raw activation command.
    let handle = support::new_handle();
    let (queue, submission, bootstrap_bytes) = support::bootstrap(&handle).await?;
    let bootstrap_status = support::queue_status(&handle, queue).await?;

    // When: descriptor bytes are written before ordinary producer publication.
    handle.write_memory(submission.base(), &[0x6b; 64]).await?;
    support::publish(&handle, queue, RingCursor::new(0)).await?;
    let submission_bytes = support::response_bytes(&handle).await?;
    let submission_status = support::queue_status(&handle, queue).await?;

    // Then: raw response reads use the exact canonical 72-byte codec.
    assert_eq!(
        bootstrap_bytes,
        support::response(bootstrap_status).to_bytes()?
    );
    assert_eq!(
        submission_bytes,
        support::response(submission_status).to_bytes()?
    );
    assert_eq!(
        submission_status.submission().producer(),
        RingCursor::new(1)
    );
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        vec![0x6b; 64]
    );
    assert_eq!(handle.memory_generation().await, 1);
    Ok(())
}

#[tokio::test]
async fn indirect_bytes_commit_before_descriptor_and_publication() -> TestResult {
    // Given: an active queue and a separately committed indirect mapping.
    let handle = support::new_handle();
    let (queue, submission, _) = support::bootstrap(&handle).await?;
    let region = handle.allocate_memory_region(16).await?;

    // When: indirect data and the descriptor are separate writes preceding publication.
    handle.write_memory(region.base(), &[0x33; 16]).await?;
    handle.write_memory(submission.base(), &[0x44; 64]).await?;
    support::publish(&handle, queue, RingCursor::new(0)).await?;

    // Then: both completed writes survive with one published descriptor.
    assert_eq!(handle.read_memory(region.base(), 16).await?, vec![0x33; 16]);
    assert_eq!(
        handle.read_memory(submission.base(), 64).await?,
        vec![0x44; 64]
    );
    assert_eq!(
        support::queue_status(&handle, queue)
            .await?
            .submission()
            .producer(),
        RingCursor::new(1)
    );
    assert_eq!(handle.memory_generation().await, 2);
    Ok(())
}

#[tokio::test]
async fn observer_records_every_register_access_in_order() -> TestResult {
    // Given: one observed endpoint and separately committed ring memory.
    let (handle, observer) = support::observed_handle();

    // When: bootstrap and publication use the ordinary numeric register surface.
    let (queue, submission, _) = support::bootstrap(&handle).await?;
    handle.write_memory(submission.base(), &[0x6b; 64]).await?;
    support::publish(&handle, queue, RingCursor::new(0)).await?;

    // Then: request writes, command writes, and response reads each own an observation.
    let observations = observer.snapshot();
    let expected_offsets = [
        0x100, 0x108, 0x130, 0x138, 0x148, 0x158, 0x120, 0x124, 0x140, 0x150, 0x160, 0x164, 0x188,
        0x190, 0x198, 0x1a0, 0x1a8, 0x1b0, 0x1b8, 0x1c0, 0x1c8, 0x100, 0x108, 0x110, 0x118, 0x164,
    ];
    assert_eq!(observations.len(), expected_offsets.len());
    for (sequence, (observation, offset)) in observations.iter().zip(expected_offsets).enumerate() {
        assert_eq!(observation.sequence, u64::try_from(sequence)?);
        assert_eq!(observation.origin, AccessOrigin::Direct);
        assert_eq!(observation.offset, offset);
    }
    let commands: Vec<_> = observations
        .iter()
        .filter(|item| item.offset == 0x164)
        .map(|item| item.request_bytes.clone())
        .collect();
    assert_eq!(
        commands,
        vec![1_u32.to_le_bytes().to_vec(), 3_u32.to_le_bytes().to_vec()]
    );
    Ok(())
}
