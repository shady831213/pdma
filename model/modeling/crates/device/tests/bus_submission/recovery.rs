use super::common::{TestResult, activate_on, allocate, encode, fixture};
use pdma_model_contract::{
    AbiCommand, AbiCopy, AbiId, CopyIovas, QueueControl, QueueCursorUpdate, RingCursor,
    WholeDeviceReset,
};
use pdma_model_contract::{
    CommandId, CompletionStatus, EventRouting, ExternalCommand, QueueConfig, ResourceConfig,
    SoftwareOwnerId,
    desc::{CopyCommand, CopyRequest},
};
use pdma_model_device::{DmaWrite, PendingDmaResult, SubmissionAcceptance};

pub(super) async fn reset_discards_pending_dma_retirement() -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let descriptor_address = fixture.submission;
    let source = allocate(&fixture.handle, 4).await?;
    let destination = allocate(&fixture.handle, 4).await?;
    fixture
        .handle
        .write_iova(source.base(), &[7, 6, 5, 4])
        .await?;
    let descriptor = encode(AbiCommand::Copy(AbiCopy::new(
        AbiId::new(60),
        CopyIovas::new(source.base(), destination.base()),
        4,
    )))?;
    fixture
        .handle
        .accept_submission(SubmissionAcceptance::new(
            fixture.queue,
            RingCursor::new(0),
            DmaWrite::new(descriptor_address, descriptor.to_vec()),
            None,
        ))
        .await?;
    fixture
        .handle
        .control_queue(QueueControl::PublishCompletionProducer(
            QueueCursorUpdate::new(fixture.queue, RingCursor::new(2)),
        ))
        .await?;
    fixture
        .handle
        .control_queue(QueueControl::CaptureSubmissionProducer(
            QueueCursorUpdate::new(fixture.queue, RingCursor::new(1)),
        ))
        .await?;
    fixture.handle.wait_for_stall_after(0).await;
    assert!(matches!(
        fixture.handle.pending_dma_result().await,
        Some(PendingDmaResult::Completion(_))
    ));

    let epoch = fixture
        .handle
        .reset(WholeDeviceReset::new(
            pdma_model_contract::semantic::DeviceEpoch::initial(),
        ))
        .await?;
    assert_eq!(fixture.handle.pending_dma_result().await, None);
    assert_eq!(fixture.handle.irq_snapshot().await, None);
    assert_eq!(
        fixture.handle.read_iova(descriptor_address, 64).await?,
        descriptor
    );
    let (new_queue, _) =
        activate_on(&fixture.handle, epoch, pdma_model_contract::QueueId::new(0)).await?;
    assert_eq!(new_queue.epoch(), epoch);
    let status = fixture
        .handle
        .control_queue(QueueControl::ReadStatus(new_queue))
        .await?;
    assert_eq!(status.submission().consumer(), RingCursor::new(0));
    fixture.handle.request_shutdown().await;
    fixture.worker.await?;
    Ok(())
}

pub(super) async fn dma_and_semantic_queues_coexist() -> TestResult {
    let fixture = fixture(ResourceConfig::default().with_compute_slots(0)).await?;
    let semantic = fixture
        .handle
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(1),
        ))
        .await?;
    let source_a = allocate(&fixture.handle, 4).await?;
    let source_b = allocate(&fixture.handle, 4).await?;
    let dma_destination = allocate(&fixture.handle, 4).await?;
    let semantic_destination = allocate(&fixture.handle, 4).await?;
    fixture
        .handle
        .write_iova(source_a.base(), &[1, 2, 3, 4])
        .await?;
    fixture
        .handle
        .write_iova(source_b.base(), &[4, 3, 2, 1])
        .await?;
    let vector = pdma_model_contract::abi::AbiVectorAdd::new(
        AbiId::new(70),
        pdma_model_contract::abi::VectorIovas::new(
            source_a.base(),
            source_b.base(),
            dma_destination.base(),
        ),
        4,
    );
    fixture
        .handle
        .accept_submission(SubmissionAcceptance::new(
            fixture.queue,
            RingCursor::new(0),
            DmaWrite::new(
                fixture.submission,
                encode(AbiCommand::VectorAdd(vector))?.to_vec(),
            ),
            None,
        ))
        .await?;
    fixture
        .handle
        .control_queue(QueueControl::CaptureSubmissionProducer(
            QueueCursorUpdate::new(fixture.queue, RingCursor::new(1)),
        ))
        .await?;

    fixture
        .handle
        .submit_command(
            semantic,
            ExternalCommand::Copy(CopyCommand::new(
                CommandId::new(71),
                semantic.queue_id(),
                semantic.completion_ring_id(),
                CopyRequest::new(
                    source_a.object(),
                    semantic_destination.object(),
                    usize::try_from(source_a.length())?,
                ),
            )),
        )
        .await?;
    fixture.handle.ring_doorbell(semantic).await?;
    fixture.handle.wait_for_stall_after(0).await;
    fixture.handle.write_iova(fixture.submission, &[]).await?;
    fixture.handle.wait_for_progress_after(0).await;
    assert_semantic_completion(&fixture.handle, semantic).await?;
    let dma_status = fixture
        .handle
        .control_queue(QueueControl::ReadStatus(fixture.queue))
        .await?;
    assert_eq!(dma_status.submission().consumer(), RingCursor::new(0));
    assert_eq!(
        fixture
            .handle
            .read_iova(dma_destination.base(), dma_destination.length())
            .await?,
        [0; 4]
    );
    assert_eq!(
        fixture
            .handle
            .read_iova(semantic_destination.base(), semantic_destination.length())
            .await?,
        [1, 2, 3, 4]
    );
    fixture.handle.request_shutdown().await;
    fixture.worker.await?;
    Ok(())
}

async fn assert_semantic_completion(
    handle: &pdma_model_device::AsyncDeviceHandle,
    queue: pdma_model_contract::QueueConfigHandle,
) -> TestResult {
    assert_eq!(
        handle.poll_completion(queue).await?,
        Some(pdma_model_contract::CompletionRecord::new(
            queue.queue_id(),
            CommandId::new(71),
            CompletionStatus::Success,
        ))
    );
    Ok(())
}
