use pdma_model_contract::abi::{AbiCommand, AbiEvent, DmaAddress};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue, RingCursor};
use pdma_model_contract::semantic::{
    EventRouting, QueueActivation, QueueConfig, QueueControl, QueueCursorUpdate, QueueId,
    QueueRingObjects, ResourceConfig, SoftwareOwnerId,
};
use pdma_model_device::{
    AsyncDevice, AsyncDeviceHandle, DmaWrite, MappedDmaRegion, PendingDmaResult,
    SubmissionAcceptance,
};

pub(in crate::queue) type TestResult<T = ()> =
    Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub(in crate::queue) struct Fixture {
    pub(in crate::queue) handle: AsyncDeviceHandle,
    pub(in crate::queue) queue: EpochQueue,
    pub(in crate::queue) submission: DmaAddress,
    pub(in crate::queue) worker: tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary>,
}

pub(in crate::queue) struct RunResult {
    pub(in crate::queue) consumer: RingCursor,
    pub(in crate::queue) pending: Option<PendingDmaResult>,
    pub(in crate::queue) destinations: Vec<Vec<u8>>,
    pub(in crate::queue) steps: usize,
    pub(in crate::queue) irq_pending: bool,
    pub(in crate::queue) traces: Vec<pdma_model_contract::semantic::TraceEvent>,
    pub(in crate::queue) native_steps: usize,
}

pub(in crate::queue) async fn fixture(resources: ResourceConfig) -> TestResult<Fixture> {
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        resources,
    ));
    let handle = runner.handle();
    let (queue, submission) = activate(&handle, QueueId::new(0)).await?;
    Ok(Fixture {
        handle,
        queue,
        submission,
        worker: tokio::spawn(worker.run()),
    })
}

pub(in crate::queue) async fn activate(
    handle: &AsyncDeviceHandle,
    queue_id: QueueId,
) -> TestResult<(EpochQueue, DmaAddress)> {
    activate_on(handle, DeviceEpoch::initial(), queue_id).await
}

pub(in crate::queue) async fn activate_on(
    handle: &AsyncDeviceHandle,
    epoch: DeviceEpoch,
    queue_id: QueueId,
) -> TestResult<(EpochQueue, DmaAddress)> {
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, completion, event] = stage.regions() else {
        return Err("ring stage shape".into());
    };
    let (submission, completion, event) = (*submission, *completion, *event);
    let config = QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0));
    let activation = QueueActivation::new(
        epoch,
        queue_id,
        config,
        QueueRingObjects::new(submission.object(), completion.object(), event.object()),
    );
    handle.activate_queue(stage, activation).await?;
    Ok((activation.queue(), submission.base()))
}

pub(in crate::queue) async fn allocate(
    handle: &AsyncDeviceHandle,
    length: usize,
) -> TestResult<MappedDmaRegion> {
    let object = handle.allocate_buffer(length).await?;
    handle
        .region_for_object(object)
        .await
        .ok_or_else(|| "committed DMA region missing".into())
}

pub(in crate::queue) async fn run_descriptor(
    fixture: Fixture,
    bytes: [u8; 64],
    destinations: &[MappedDmaRegion],
) -> TestResult<RunResult> {
    fixture.handle.enable_tracing().await;
    fixture
        .handle
        .control_queue(QueueControl::PublishCompletionProducer(
            QueueCursorUpdate::new(fixture.queue, RingCursor::new(2)),
        ))
        .await?;
    fixture
        .handle
        .control_queue(QueueControl::PublishEventProducer(QueueCursorUpdate::new(
            fixture.queue,
            RingCursor::new(2),
        )))
        .await?;
    fixture
        .handle
        .accept_submission(SubmissionAcceptance::new(
            fixture.queue,
            RingCursor::new(0),
            DmaWrite::new(fixture.submission, bytes.to_vec()),
            None,
        ))
        .await?;
    fixture
        .handle
        .control_queue(QueueControl::CaptureSubmissionProducer(
            QueueCursorUpdate::new(fixture.queue, RingCursor::new(1)),
        ))
        .await?;
    fixture.handle.wait_for_stall_after(0).await;
    fixture.handle.write_memory(fixture.submission, &[]).await?;
    fixture.handle.wait_for_stall_after(1).await;
    let status = fixture
        .handle
        .control_queue(QueueControl::ReadStatus(fixture.queue))
        .await?;
    let pending = fixture.handle.pending_dma_result().await;
    let snapshot = fixture.handle.snapshot().await;
    let irq_pending = fixture.handle.irq_snapshot().await.is_some();
    let traces = fixture.handle.trace_events().await;
    let native_steps = fixture.handle.native_steps().await;
    let mut bytes = Vec::new();
    for region in destinations {
        bytes.push(
            fixture
                .handle
                .read_memory(region.base(), region.length())
                .await?,
        );
    }
    fixture.handle.request_shutdown().await;
    fixture.worker.await?;
    Ok(RunResult {
        consumer: status.submission().consumer(),
        pending,
        destinations: bytes,
        steps: snapshot.steps,
        irq_pending,
        traces,
        native_steps,
    })
}

pub(in crate::queue) fn encode(command: AbiCommand) -> TestResult<[u8; 64]> {
    let mut bytes = [0_u8; 64];
    command.encode(&mut bytes)?;
    Ok(bytes)
}

pub(in crate::queue) fn pending_event(result: &RunResult) -> TestResult<AbiEvent> {
    match result.pending {
        Some(PendingDmaResult::Event(event)) => Ok(event),
        Some(PendingDmaResult::Completion(_)) | None => Err("expected pending DMA event".into()),
    }
}
