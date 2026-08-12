use super::*;
use pdma_model_contract::{
    CompletionRecord, CopyRequest, EpochIrqSource, EventRecord, IrqNotification, IrqRoute,
    IrqSnapshot, QueueConfig, QueueConfigHandle,
};
use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;
use std::future::{Future, pending, poll_fn};
use std::task::Poll;

#[path = "driver_credit/pending_doorbell.rs"]
mod pending_doorbell;

use pending_doorbell::PendingDoorbellPort;

pub(super) async fn verify_after_acceptance_driver_credit() -> ModelResult<()> {
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let cpu = SoftwareCpu::new();
    let (mut port, mut driver, mut retained, source, destination_a, destination_b) = cpu
        .run_main(async {
            let raw = BusDevicePort::new(device.handle())?;
            let mut port = PendingDoorbellPort::new(raw);
            let driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let retained = driver.clone();
            let source = driver.allocate_buffer_on(&mut port, 4).await?;
            let destination_a = driver.allocate_buffer_on(&mut port, 4).await?;
            let destination_b = driver.allocate_buffer_on(&mut port, 4).await?;
            Ok::<_, ModelError>((port, driver, retained, source, destination_a, destination_b))
        })
        .await?;
    let queue = driver.queue_config_handle();
    let epoch_queue = EpochQueue::new(queue.epoch(), queue.queue_id());
    let submission = region(&handle, BufferId::new(0)).await?;
    let request = BroadcastCopyRequest::new(
        source,
        0,
        4,
        vec![
            BroadcastDestinationRange::new(destination_a, 0),
            BroadcastDestinationRange::new(destination_b, 0),
        ],
    );

    let doorbell_pending = cpu
        .run_main(async {
            let mut submission = Box::pin(driver.submit_broadcast_copy_on(&mut port, request));
            let pending = poll_fn(|context| match submission.as_mut().poll(context) {
                Poll::Pending => Poll::Ready(true),
                Poll::Ready(_) => Poll::Ready(false),
            })
            .await;
            drop(submission);
            Ok::<_, ModelError>(pending)
        })
        .await?;
    assert!(doorbell_pending);
    let committed = submission_state(&handle, epoch_queue, submission).await?;
    let source_region = region(&handle, source).await?;
    let destinations = [
        region(&handle, destination_a).await?,
        region(&handle, destination_b).await?,
    ];
    let (expected_descriptor_bytes, expected_list_bytes) =
        encode_broadcast(source_region, destinations, committed.indirect)?;
    let next = cpu
        .run_main(retained.submit_copy_on(&mut port, CopyRequest::new(source, destination_a, 4)))
        .await;

    assert_eq!(committed.status.submission().producer().raw(), 1);
    assert_eq!(committed.status.captured_submission_producer().raw(), 0);
    assert_eq!(committed.list_bytes, expected_list_bytes);
    assert_eq!(committed.descriptor_bytes, expected_descriptor_bytes);
    assert_eq!(next, Err(ModelError::DriverQueueFull));
    Ok(())
}

pub(super) async fn verify_before_acceptance_driver_credit() -> ModelResult<()> {
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let cpu = SoftwareCpu::new();
    let (mut port, mut driver, mut retained, source, destination) = cpu
        .run_main(async {
            let raw = BusDevicePort::new(device.handle())?;
            let mut port = PendingDoorbellPort::with_pending_submission(raw);
            let driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let retained = driver.clone();
            let source = driver.allocate_buffer_on(&mut port, 4).await?;
            let destination = driver.allocate_buffer_on(&mut port, 4).await?;
            Ok::<_, ModelError>((port, driver, retained, source, destination))
        })
        .await?;
    let queue = driver.queue_config_handle();
    let epoch_queue = EpochQueue::new(queue.epoch(), queue.queue_id());
    let submission = region(&handle, BufferId::new(0)).await?;
    let before = AsyncPdmaBus::read_queue_status(&handle, epoch_queue)
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let before_bytes = dma_bytes(
        &handle,
        submission.base(),
        u64::from(RingRecord::Submission.record_size()),
    )
    .await?;
    let source_region = region(&handle, source).await?;
    let destination_region = region(&handle, destination).await?;

    let pending_acceptance = cpu
        .run_main(async {
            let mut submit = Box::pin(
                driver.submit_copy_on(&mut port, CopyRequest::new(source, destination, 4)),
            );
            let pending = poll_fn(|context| match submit.as_mut().poll(context) {
                Poll::Pending => Poll::Ready(true),
                Poll::Ready(_) => Poll::Ready(false),
            })
            .await;
            drop(submit);
            Ok::<_, ModelError>(pending)
        })
        .await?;
    assert!(pending_acceptance);
    assert_eq!(
        AsyncPdmaBus::read_queue_status(&handle, epoch_queue).await,
        Ok(before)
    );
    assert_eq!(
        dma_bytes(
            &handle,
            submission.base(),
            u64::from(RingRecord::Submission.record_size()),
        )
        .await?,
        before_bytes
    );
    assert_eq!(region(&handle, source).await?, source_region);
    assert_eq!(region(&handle, destination).await?, destination_region);

    port.release_submission();
    assert_eq!(
        cpu.run_main(retained.submit_copy_on(&mut port, CopyRequest::new(source, destination, 4)))
            .await?,
        CommandId::new(1)
    );
    Ok(())
}
