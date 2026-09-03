use std::num::NonZeroUsize;

use pdma_model_contract::semantic::{
    BufferId, Command, CommandId, CommandOperation, EpochIrqSource, IrqSource, QueueConfigHandle,
    ResourceConfig, TraceEvent, TraceEventKind,
    desc::{
        BroadcastCopyRequest, BroadcastDestinationRange, CopyRequest, StridedCopyRequest,
        VectorAddRequest, VectorBufferRange,
    },
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::{
    MappedDmaRegion, ModelError, PipelineFixture, PipelineRunOutcome, PipelineTerminal,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    Copy,
    Strided,
    Broadcast,
    VectorAdd,
    UnknownBuffer,
    RangeOverflow,
    OutOfBounds,
}

impl Case {
    pub(super) const ALL: [Self; 7] = [
        Self::Copy,
        Self::Strided,
        Self::Broadcast,
        Self::VectorAdd,
        Self::UnknownBuffer,
        Self::RangeOverflow,
        Self::OutOfBounds,
    ];
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Observation {
    pub(super) terminal: Result<PipelineTerminal, ModelError>,
    pub(super) destinations: [Vec<u8>; 2],
    pub(super) completion: Option<pdma_model_contract::semantic::CompletionRecord>,
    pub(super) event: Option<pdma_model_contract::semantic::EventRecord>,
    pub(super) irq: bool,
    pub(super) trace: Vec<TraceEvent>,
}

pub(super) struct Prepared {
    pub(super) fixture: PipelineFixture,
    pub(super) queue: QueueConfigHandle,
    pub(super) destinations: [MappedDmaRegion; 2],
}

#[derive(Clone, Copy)]
pub(super) enum Drive {
    Legacy,
    Stepped,
    Immediate,
}

pub(super) fn prepare(case: Case) -> Prepared {
    prepare_with_resources(case, ResourceConfig::default())
}

pub(super) fn prepare_with_resources(case: Case, resources: ResourceConfig) -> Prepared {
    let mut fixture = PipelineFixture::with_resources(resources);
    fixture.enable_tracing();
    let source_a = fixture.allocate(8).expect("source A");
    let source_b = fixture.allocate(8).expect("source B");
    let destination_a = fixture.allocate(8).expect("destination A");
    let destination_b = fixture.allocate(8).expect("destination B");
    fixture
        .write(source_a, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("source A bytes");
    fixture
        .write(source_b, &[8, 7, 6, 5, 4, 3, 2, 1])
        .expect("source B bytes");
    let queue = fixture
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue");
    fixture
        .submit(
            queue,
            command(case, source_a, source_b, destination_a, destination_b),
        )
        .expect("submission");
    fixture.doorbell(queue).expect("doorbell");
    let source = EpochIrqSource::new(
        queue.epoch(),
        IrqSource::Completion(queue.completion_ring_id()),
    );
    fixture.enable_irq_source(source).expect("enable IRQ");
    fixture.unmask_irq_source(source).expect("unmask IRQ");
    Prepared {
        fixture,
        queue,
        destinations: [destination_a, destination_b],
    }
}

pub(super) fn prepared_pair() -> Prepared {
    let mut prepared = prepare_with_resources(
        Case::Copy,
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    let source = prepared.fixture.allocate(8).expect("second source");
    prepared
        .fixture
        .write(source, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("second source bytes");
    prepared
        .fixture
        .submit(
            prepared.queue,
            Command::new(
                CommandId::new(99),
                CommandOperation::Copy(CopyRequest::new(
                    source.object(),
                    prepared.destinations[1].object(),
                    4,
                )),
            ),
        )
        .expect("second submission");
    prepared
}

fn command(
    case: Case,
    a: MappedDmaRegion,
    b: MappedDmaRegion,
    x: MappedDmaRegion,
    y: MappedDmaRegion,
) -> Command {
    let operation = match case {
        Case::Copy => CommandOperation::Copy(CopyRequest::new(a.object(), x.object(), 4)),
        Case::Strided => CommandOperation::StridedCopy(StridedCopyRequest::new(
            a.object(),
            x.object(),
            2,
            2,
            3,
            3,
        )),
        Case::Broadcast => CommandOperation::BroadcastCopy(BroadcastCopyRequest::new(
            a.object(),
            1,
            3,
            vec![
                BroadcastDestinationRange::new(x.object(), 0),
                BroadcastDestinationRange::new(y.object(), 2),
            ],
        )),
        Case::VectorAdd => CommandOperation::VectorAdd(VectorAddRequest::new(
            VectorBufferRange::new(a.object(), 0),
            VectorBufferRange::new(b.object(), 0),
            VectorBufferRange::new(x.object(), 0),
            4,
        )),
        Case::UnknownBuffer => {
            CommandOperation::Copy(CopyRequest::new(BufferId::new(999), x.object(), 4))
        }
        Case::RangeOverflow => CommandOperation::Copy(CopyRequest::with_starts(
            pdma_model_contract::semantic::DmaRangeStart::new(a.object(), usize::MAX),
            pdma_model_contract::semantic::DmaRangeStart::new(x.object(), 0),
            2,
        )),
        Case::OutOfBounds => CommandOperation::Copy(CopyRequest::new(a.object(), x.object(), 9)),
    };
    Command::new(CommandId::new(7), operation)
}

pub(super) fn observe(mut prepared: Prepared, drive: Drive) -> Result<Observation, String> {
    let terminal = match drive {
        Drive::Legacy => prepared.fixture.legacy_step(),
        Drive::Stepped => drive_to_complete(&mut prepared.fixture)?,
        Drive::Immediate => match prepared
            .fixture
            .run_immediate(NonZeroUsize::new(8).expect("budget"))
        {
            PipelineRunOutcome::Complete(result) => result,
            other => return Err(format!("unexpected immediate result: {other:?}")),
        },
    };
    let trace = prepared.fixture.trace_events();
    Ok(Observation {
        terminal,
        destinations: [
            prepared
                .fixture
                .read(prepared.destinations[0], 8)
                .expect("destination A"),
            prepared
                .fixture
                .read(prepared.destinations[1], 8)
                .expect("destination B"),
        ],
        completion: prepared
            .fixture
            .poll_completion(prepared.queue)
            .expect("completion"),
        event: prepared.fixture.poll_event(prepared.queue).expect("event"),
        irq: prepared.fixture.irq_snapshot().is_some(),
        trace,
    })
}

pub(super) fn drive_to_pending(fixture: &mut PipelineFixture) -> PipelineRunOutcome {
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            outcome => return outcome,
        }
    }
}

pub(super) fn drive_to_complete(
    fixture: &mut PipelineFixture,
) -> Result<Result<PipelineTerminal, ModelError>, String> {
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            PipelineRunOutcome::Complete(result) => return Ok(result),
            other => return Err(format!("unexpected stepped result: {other:?}")),
        }
    }
}

pub(super) fn count(trace: &[TraceEvent], kind: TraceEventKind) -> usize {
    trace.iter().filter(|event| event.kind() == kind).count()
}
