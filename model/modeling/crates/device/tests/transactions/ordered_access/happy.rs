use super::support::{self, TestResult};
use hardware_interface_runtime::AccessOrigin;
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};

#[tokio::test]
async fn individual_accesses_publish_direct_queue_cursors() -> TestResult {
    // Given: three independently committed rings and a directly enabled queue.
    let handle = support::new_handle();
    let submission = support::bootstrap(&handle).await?;
    let bootstrap_cursors = support::cursors(&handle).await?;

    // When: descriptor bytes are written before ordinary producer publication.
    handle.write_memory(submission, &[0x6b; 64]).await?;
    support::publish(&handle, 1).await?;
    let submission_cursors = support::cursors(&handle).await?;

    // Then: producer publication changes only its live cursor, without capturing work.
    assert_eq!(bootstrap_cursors, vec![0; 7]);
    assert_eq!(submission_cursors, vec![1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(handle.read_memory(submission, 64).await?, vec![0x6b; 64]);
    assert_eq!(handle.memory.revision().raw(), 3);
    Ok(())
}

#[tokio::test]
async fn indirect_bytes_commit_before_descriptor_and_publication() -> TestResult {
    // Given: an active queue and a separately committed indirect mapping.
    let handle = support::new_handle();
    let submission = support::bootstrap(&handle).await?;
    let region = handle.allocate(16)?;

    // When: indirect data and the descriptor are separate writes preceding publication.
    handle.write_memory(region, &[0x33; 16]).await?;
    handle.write_memory(submission, &[0x44; 64]).await?;
    support::publish(&handle, 1).await?;

    // Then: both completed writes survive with one published descriptor.
    assert_eq!(handle.read_memory(region, 16).await?, vec![0x33; 16]);
    assert_eq!(handle.read_memory(submission, 64).await?, vec![0x44; 64]);
    assert_eq!(
        support::read_cursor(&handle, QueueRegisters::SQ_PRODUCER.offset()).await?,
        1
    );
    assert_eq!(handle.memory.revision().raw(), 4);
    Ok(())
}

#[tokio::test]
async fn observer_records_every_register_access_in_order() -> TestResult {
    // Given: one observed endpoint and separately committed ring memory.
    let (handle, observer) = support::observed_handle();

    // When: bootstrap and publication use the ordinary numeric register surface.
    let submission = support::bootstrap(&handle).await?;
    handle.write_memory(submission, &[0x6b; 64]).await?;
    support::publish(&handle, 1).await?;

    // Then: configuration, enable, and producer writes each own an observation.
    let observations = observer.snapshot();
    let expected_offsets = [
        QueueRegisters::SQ_BASE.offset(),
        QueueRegisters::CQ_BASE.offset(),
        QueueRegisters::EQ_BASE.offset(),
        QueueRegisters::SQ_DEPTH.offset(),
        QueueRegisters::CQ_DEPTH.offset(),
        QueueRegisters::EQ_DEPTH.offset(),
        QueueRegisters::ENABLE.offset(),
        QueueRegisters::SQ_PRODUCER.offset(),
    ];
    assert_eq!(observations.len(), expected_offsets.len());
    for (sequence, (observation, offset)) in observations.iter().zip(expected_offsets).enumerate() {
        assert_eq!(observation.sequence, u64::try_from(sequence)?);
        assert_eq!(observation.origin, AccessOrigin::Direct);
        assert_eq!(observation.offset, queue_offset(0, offset)?);
    }
    assert_eq!(
        observations
            .last()
            .ok_or("producer observation")?
            .request_bytes,
        1_u64.to_le_bytes()
    );
    Ok(())
}
