use pdma_model_contract::abi::{AbiCommand, DmaAddress};
use pdma_model_contract::abi::{RingCursor, RingRecord};
use pdma_model_contract::binding::{
    AbiEvent, PdmaMemoryRegion, QueueActivation, QueueControl, QueueCursorUpdate, QueueRingRegions,
    RingLayout,
};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue};
use pdma_model_contract::semantic::{
    EventRouting, QueueConfig, QueueId, ResourceConfig, SoftwareOwnerId,
};
use pdma_model_device::{
    AsyncDevice, AsyncDeviceHandle, DmaWrite, PendingDmaResult, SubmissionAcceptance,
};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub(super) struct Fixture {
    pub(super) handle: AsyncDeviceHandle,
    pub(super) queue: EpochQueue,
    pub(super) submission: DmaAddress,
    pub(super) worker: tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary>,
}

pub(super) struct RunResult {
    pub(super) consumer: RingCursor,
    pub(super) pending: Option<PendingDmaResult>,
    pub(super) destinations: Vec<Vec<u8>>,
    pub(super) steps: usize,
    pub(super) irq_pending: bool,
    pub(super) traces: Vec<pdma_model_contract::semantic::TraceEvent>,
    pub(super) native_steps: usize,
}

pub(super) async fn fixture(resources: ResourceConfig) -> TestResult<Fixture> {
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        resources,
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
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

pub(super) async fn activate(
    handle: &AsyncDeviceHandle,
    queue_id: QueueId,
) -> TestResult<(EpochQueue, DmaAddress)> {
    activate_on(handle, DeviceEpoch::initial(), queue_id).await
}

pub(super) async fn activate_on(
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
    let activation = QueueActivation::try_new(
        epoch,
        queue_id,
        config,
        QueueRingRegions::new(
            RingLayout::try_new(submission, 2, RingRecord::Submission)?,
            RingLayout::try_new(completion, 2, RingRecord::Completion)?,
            RingLayout::try_new(event, 2, RingRecord::Event)?,
        ),
    )?;
    handle.activate_queue(stage, activation).await?;
    Ok((activation.queue(), submission.base()))
}

pub(super) async fn allocate(
    handle: &AsyncDeviceHandle,
    length: usize,
) -> TestResult<PdmaMemoryRegion> {
    let object = handle.allocate_buffer(length).await?;
    handle
        .region_for_object(object)
        .await
        .ok_or_else(|| "committed DMA region missing".into())
}

pub(super) async fn run_descriptor(
    fixture: Fixture,
    bytes: [u8; 64],
    destinations: &[PdmaMemoryRegion],
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

pub(super) fn encode(command: AbiCommand) -> TestResult<[u8; 64]> {
    let mut bytes = [0_u8; 64];
    command.encode(&mut bytes)?;
    Ok(bytes)
}

pub(super) fn pending_event(result: &RunResult) -> TestResult<AbiEvent> {
    match result.pending {
        Some(PendingDmaResult::Event(event)) => Ok(event),
        Some(PendingDmaResult::Completion(_)) | None => Err("expected pending DMA event".into()),
    }
}
