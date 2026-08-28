use super::common::{TestResult, activate, allocate, encode};
use pdma_model_contract::abi::{AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues};
use pdma_model_contract::semantic::{QueueControl, QueueCursorUpdate, ResourceConfig, RingCursor};
use pdma_model_device::{AsyncDevice, DmaWrite, PendingDmaResult, SubmissionAcceptance};

pub(super) async fn capture_and_nonzero_slot() -> TestResult {
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let handle = runner.handle();
    let (queue, submission) =
        activate(&handle, pdma_model_contract::semantic::QueueId::new(0)).await?;
    let source = allocate(&handle, 4).await?;
    let destination = allocate(&handle, 4).await?;
    handle.write_memory(source.base(), &[4, 3, 2, 1]).await?;

    let mut poisoned = [0_u8; 64];
    poisoned[0..2].copy_from_slice(&2_u16.to_le_bytes());
    handle
        .accept_submission(SubmissionAcceptance::new(
            queue,
            RingCursor::new(0),
            DmaWrite::new(submission, poisoned.to_vec()),
            None,
        ))
        .await?;
    assert_eq!(handle.snapshot().await.steps, 0);
    assert_eq!(handle.pending_dma_result().await, None);

    handle
        .control_queue(QueueControl::PublishSubmissionConsumer(
            QueueCursorUpdate::new(queue, RingCursor::new(1)),
        ))
        .await?;
    let second_slot = submission.checked_add(64)?;
    let command = encode(AbiCommand::from_payload(
        50,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: source.base().raw(),
            destination_dma_address: destination.base().raw(),
            length: 4.into(),
        }),
    ))?;
    handle
        .accept_submission(SubmissionAcceptance::new(
            queue,
            RingCursor::new(1),
            DmaWrite::new(second_slot, command.to_vec()),
            None,
        ))
        .await?;
    let ring = handle.read_memory(submission, 128).await?;
    assert_eq!(ring.get(0..64), Some(poisoned.as_slice()));
    assert_eq!(ring.get(64..128), Some(command.as_slice()));

    handle
        .control_queue(QueueControl::CaptureSubmissionProducer(
            QueueCursorUpdate::new(queue, RingCursor::new(2)),
        ))
        .await?;
    handle
        .control_queue(QueueControl::PublishCompletionProducer(
            QueueCursorUpdate::new(queue, RingCursor::new(2)),
        ))
        .await?;
    let worker = tokio::spawn(worker.run());
    handle.wait_for_stall_after(0).await;
    let status = handle
        .control_queue(QueueControl::ReadStatus(queue))
        .await?;
    assert_eq!(status.submission().consumer(), RingCursor::new(2));
    assert_eq!(
        handle.pending_dma_result().await,
        Some(PendingDmaResult::Completion(
            pdma_model_contract::semantic::CompletionRecord::new(
                queue.queue_id(),
                pdma_model_contract::semantic::CommandId::new(50),
                pdma_model_contract::semantic::CompletionStatus::Success,
            )
        ))
    );
    assert_eq!(
        handle
            .read_memory(destination.base(), destination.length())
            .await?,
        [4, 3, 2, 1]
    );
    assert_eq!(handle.irq_snapshot().await, None);
    handle.request_shutdown().await;
    worker.await?;
    Ok(())
}
