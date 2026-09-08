#![allow(missing_docs)]
#![allow(clippy::expect_used, reason = "checked test fixtures")]

use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
    DmaAddress, RingLayout,
};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochIrqSource, EpochQueue, EventRouting, IrqSource, QueueActivation, QueueConfig,
    QueueControl, QueueCursorUpdate, QueueId, QueueRingObjects, ResourceConfig, RingCursor,
    SoftwareOwnerId, TraceEvent, TraceEventKind, WholeDeviceReset,
};
use pdma_model_device::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncStallReason, AsyncStepStatus,
    PendingDmaResult,
};

struct Fixture {
    handle: AsyncDeviceHandle,
    worker: Option<AsyncDeviceWorker>,
    queue: EpochQueue,
    submission: DmaAddress,
}

impl Fixture {
    async fn new(resources: ResourceConfig) -> Self {
        let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            resources,
        ));
        let handle = device.handle();
        let stage = handle
            .stage_memory_regions(&[
                RingLayout::<AbiCommand>::checked_extent(4).expect("command extent"),
                RingLayout::<AbiCompletion>::checked_extent(1).expect("completion extent"),
                RingLayout::<AbiEvent>::checked_extent(1).expect("event extent"),
            ])
            .await
            .expect("ring stage");
        let submission = *stage.regions().first().expect("submission region");
        let completion = *stage.regions().get(1).expect("completion region");
        let event = *stage.regions().last().expect("event region");
        let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
        let rings = QueueRingObjects::new(submission.object(), completion.object(), event.object());
        let submission = submission.base();
        handle
            .commit_memory_stage(stage)
            .await
            .expect("ring mappings");
        handle
            .activate_mapped_queue(activation(DeviceEpoch::initial(), rings))
            .await
            .expect("queue activation");
        Self {
            handle,
            worker: Some(worker),
            queue,
            submission,
        }
    }

    async fn submit(&self, producer: u64, command: u64, source: DmaAddress, target: DmaAddress) {
        let descriptor = AbiCommand::from_payload(
            command,
            AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                source_dma_address: source.raw(),
                destination_dma_address: target.raw(),
                length: 4.into(),
            }),
        );
        let mut bytes = [0; 64];
        descriptor.encode(&mut bytes).expect("descriptor encode");
        let address = self
            .submission
            .checked_add(producer * 64)
            .expect("descriptor address");
        self.handle
            .write_memory(address, &bytes)
            .await
            .expect("descriptor write");
        self.cursor(QueueControl::PublishSubmissionProducer, producer + 1)
            .await;
    }

    async fn cursor(&self, control: fn(QueueCursorUpdate) -> QueueControl, value: u64) {
        self.handle
            .control_queue(control(QueueCursorUpdate::new(
                self.queue,
                RingCursor::new(value),
            )))
            .await
            .expect("cursor update");
    }

    fn start(&mut self) -> tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary> {
        tokio::spawn(self.worker.take().expect("worker starts once").run())
    }
}

#[tokio::test(flavor = "current_thread")]
#[rustfmt::skip]
async fn stalled_resume_retries_publication_without_refetch_or_reexecution() {
    // Given: two accepted copies, one completion slot, and an enabled completion IRQ.
    let mut fixture = Fixture::new(ResourceConfig::default().with_completion_publication_slots(1)).await;
    fixture.handle.enable_tracing().await;
    let irq = EpochIrqSource::new(DeviceEpoch::initial(), IrqSource::Completion(pdma_model_contract::semantic::CompletionRingId::new(0)));
    fixture.handle.enable_irq_source(irq).await.expect("enable IRQ");
    fixture.handle.unmask_irq_source(irq).await.expect("unmask IRQ");
    let [source, first, second] = operands(&fixture.handle).await;
    write(&fixture.handle, source.base(), &[1, 3, 5, 7]).await;
    fixture.submit(0, 20, source.base(), first.base()).await;
    fixture.submit(1, 21, source.base(), second.base()).await;
    fixture.cursor(QueueControl::CaptureSubmissionProducer, 2).await;
    let before = fixture.handle.snapshot().await;
    let worker = fixture.start();

    // When: command two stalls after DMA, its source changes, and publication space opens.
    let stalled = fixture.handle.wait_for_stall_after(0).await;
    write(&fixture.handle, source.base(), &[9; 4]).await;
    let repeated = fixture.handle.wait_for_stall_after(1).await;
    fixture.cursor(QueueControl::PublishCompletionConsumer, 1).await;
    let resumed = fixture.handle.wait_for_progress_after(1).await;
    let status = fixture.handle.control_queue(QueueControl::ReadStatus(fixture.queue)).await.expect("queue status");
    let trace = fixture.handle.trace_events().await;

    // Then: upstream work ran once, retirement alone retried, and visibility stayed ordered.
    assert_eq!((stalled.steps, stalled.stalls, stalled.errors), (1, 1, 0));
    assert_eq!(stalled.generation(), before.generation() + 2);
    assert_eq!((repeated.steps, repeated.stalls), (1, 2));
    assert_eq!(repeated.generation(), stalled.generation() + 2);
    assert_eq!(fixture.handle.native_steps().await, 2);
    assert_eq!((status.submission().consumer(), status.completion().producer(), status.completion().consumer()), (RingCursor::new(2), RingCursor::new(2), RingCursor::new(1)));
    assert_eq!(read(&fixture.handle, second.base()).await, [1, 3, 5, 7]);
    assert_eq!(count(&trace, TraceEventKind::ExecutionSucceeded), 2);
    assert_eq!(count(&trace, TraceEventKind::CompletionPublished), 2);
    assert_eq!(count(&trace, TraceEventKind::IrqSourceLatched), 2);
    assert!(adjacent(&trace, TraceEventKind::CompletionPublished, TraceEventKind::IrqSourceLatched));
    println!("stalled_resume steps=2 stalls=2 generation={} native_steps=2 cursors=2/2/1 executions=2 publications=2", resumed.generation());
    wait_idle(&fixture.handle).await;
    fixture.handle.request_shutdown().await;
    let summary = worker.await.expect("worker join");
    assert_eq!((summary.steps, summary.stalls, summary.errors), (2, 2, 0));
}

#[tokio::test(flavor = "current_thread")]
#[rustfmt::skip]
async fn reset_pending_blocks_pre_execution_and_post_execution_publication() {
    // Given: one command blocked by execution resources before DMA.
    let mut blocked = Fixture::new(ResourceConfig::default().with_read_slots(0)).await;
    blocked.handle.enable_tracing().await;
    let [source, target, _unused] = operands(&blocked.handle).await;
    write(&blocked.handle, source.base(), &[4, 3, 2, 1]).await;
    blocked.submit(0, 30, source.base(), target.base()).await;
    blocked.cursor(QueueControl::CaptureSubmissionProducer, 1).await;
    let memory_generation = blocked.handle.memory_generation().await;
    let blocked_worker = blocked.start();
    let resource_stall = blocked.handle.wait_for_stall_after(0).await;

    // When: reset advances the epoch at the execution-resource pending boundary.
    let epoch = reset(&blocked.handle).await;

    // Then: admission stalls before fetch, DMA, cursors, IRQ, or success publication.
    assert_eq!(resource_stall.last_status, AsyncStepStatus::Stalled(AsyncStallReason::ExecutionResourcePressure));
    assert_eq!((resource_stall.steps, resource_stall.stalls, resource_stall.errors), (0, 1, 0));
    assert_eq!(blocked.handle.native_steps().await, 0);
    assert_eq!(blocked.handle.memory_generation().await, memory_generation);
    assert_reset_quiescent(&blocked, epoch, &[target.base()], (vec![vec![0; 4]], [0, 0, 0], false, false)).await;
    blocked.handle.request_shutdown().await;
    let blocked_summary = blocked_worker.await.expect("blocked worker join");
    assert_eq!((blocked_summary.steps, blocked_summary.stalls, blocked_summary.errors), (0, 1, 0));

    // Given: a second command whose DMA executed before completion publication stalled.
    let mut pending = Fixture::new(ResourceConfig::default().with_completion_publication_slots(1)).await;
    pending.handle.enable_tracing().await;
    let [source, first, second] = operands(&pending.handle).await;
    write(&pending.handle, source.base(), &[0xa5, 0x5a, 0x33, 0xcc]).await;
    pending.submit(0, 40, source.base(), first.base()).await;
    pending.submit(1, 41, source.base(), second.base()).await;
    pending.cursor(QueueControl::CaptureSubmissionProducer, 2).await;
    let pending_worker = pending.start();
    let publication_stall = pending.handle.wait_for_stall_after(0).await;
    assert!(matches!(pending.handle.pending_dma_result().await, Some(PendingDmaResult::Completion(record)) if record.command_id().raw() == 41));
    let generation = pending.handle.memory_generation().await;

    // When: reset drops the post-execution pending retirement.
    let epoch = reset(&pending.handle).await;

    // Then: both DMA writes persist, but only the first completion was ever published.
    assert_eq!((publication_stall.steps, publication_stall.stalls, publication_stall.errors), (1, 1, 0));
    assert_eq!(pending.handle.native_steps().await, 2);
    assert_eq!(pending.handle.memory_generation().await, generation);
    let bytes = vec![vec![0xa5, 0x5a, 0x33, 0xcc]; 2];
    assert_reset_quiescent(&pending, epoch, &[first.base(), second.base()], (bytes, [2, 1, 0], false, false)).await;
    println!("reset_pending pre_fetch=0 pre_dma=0 post_fetch=2 post_dma=2 publications=1 steps=1 stalls=1 epoch=1 post_idle_stable=true fresh_epoch_result_empty=true");
    pending.handle.request_shutdown().await;
    let summary = pending_worker.await.expect("pending worker join");
    assert_eq!((summary.steps, summary.stalls, summary.errors), (1, 1, 0));
}

const fn activation(epoch: DeviceEpoch, rings: QueueRingObjects) -> QueueActivation {
    QueueActivation::new(
        epoch,
        QueueId::new(0),
        QueueConfig::new(4, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
        rings,
    )
}

async fn operands(handle: &AsyncDeviceHandle) -> [pdma_model_device::MappedDmaRegion; 3] {
    let stage = handle
        .stage_memory_regions(&[4; 3])
        .await
        .expect("operand stage");
    let source = *stage.regions().first().expect("source region");
    let first = *stage.regions().get(1).expect("first target region");
    let second = *stage.regions().last().expect("second target region");
    let result = [source, first, second];
    handle
        .commit_memory_stage(stage)
        .await
        .expect("operand commit");
    result
}

async fn reset(handle: &AsyncDeviceHandle) -> DeviceEpoch {
    handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await
        .expect("current epoch reset")
}

type ResetObservables = (Vec<Vec<u8>>, [usize; 3], bool, bool);

#[rustfmt::skip]
async fn reset_observables(fixture: &Fixture, addresses: &[DmaAddress]) -> ResetObservables {
    let mut bytes = Vec::with_capacity(addresses.len());
    for &address in addresses { bytes.push(read(&fixture.handle, address).await); }
    let trace = fixture.handle.trace_events().await;
    let counts = [count(&trace, TraceEventKind::ExecutionSucceeded), count(&trace, TraceEventKind::CompletionPublished), count(&trace, TraceEventKind::IrqSourceLatched)];
    (bytes, counts, fixture.handle.pending_dma_result().await.is_some(), fixture.handle.irq_snapshot().await.is_some())
}

#[rustfmt::skip]
async fn assert_reset_quiescent(fixture: &Fixture, epoch: DeviceEpoch, addresses: &[DmaAddress], expected: ResetObservables) {
    let at_reset = reset_observables(fixture, addresses).await;
    assert_eq!(at_reset, expected);
    wait_idle(&fixture.handle).await;
    assert_eq!(reset_observables(fixture, addresses).await, at_reset);
    assert!(fixture.handle.control_queue(QueueControl::ReadStatus(fixture.queue)).await.is_err());
    let fresh = fixture.handle.create_queue(QueueConfig::new(1, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0))).await.expect("new epoch queue");
    assert_eq!(fresh.epoch(), epoch);
    assert_eq!(fixture.handle.poll_completion(fresh).await.expect("fresh result path"), None);
    wait_idle(&fixture.handle).await;
    assert_eq!(reset_observables(fixture, addresses).await, at_reset);
}

async fn write(handle: &AsyncDeviceHandle, address: DmaAddress, bytes: &[u8]) {
    handle
        .write_memory(address, bytes)
        .await
        .expect("DMA write");
}

async fn read(handle: &AsyncDeviceHandle, address: DmaAddress) -> Vec<u8> {
    handle.read_memory(address, 4).await.expect("DMA read")
}

#[rustfmt::skip]
async fn wait_idle(handle: &AsyncDeviceHandle) {
    let mut snapshot = handle.snapshot().await;
    while snapshot.last_status != AsyncStepStatus::Idle { snapshot = handle.wait_for_change_after(snapshot).await; }
}

fn count(events: &[TraceEvent], kind: TraceEventKind) -> usize {
    events.iter().filter(|event| event.kind() == kind).count()
}

fn adjacent(events: &[TraceEvent], first: TraceEventKind, second: TraceEventKind) -> bool {
    events
        .windows(2)
        .any(|pair| matches!(pair, [left, right] if left.kind() == first && right.kind() == second))
}
