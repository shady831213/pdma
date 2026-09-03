use std::num::NonZeroUsize;

use hardware_interface_pipeline::{ImmediateExecutor, RunError, RunOutcome, StepExecutor};
use pdma_model_contract::semantic::{
    BufferId, Command, CommandId, CommandOperation, EpochIrqSource, IrqSource, QueueConfigHandle,
    ResourceConfig, TraceEvent,
    desc::{
        BroadcastCopyRequest, BroadcastDestinationRange, CopyRequest, StridedCopyRequest,
        VectorAddRequest, VectorBufferRange,
    },
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};

use super::super::{DeviceCore, ModelStepStatus, PdmaCommandPipeline};
use crate::{AsyncStallReason, MappedDmaRegion, ModelError, ModelResult};

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
pub(super) enum PipelineRunOutcome {
    Progressed,
    Pending(AsyncStallReason),
    Complete(Result<ModelStepStatus, ModelError>),
    Exhausted,
    BudgetExhausted,
}

#[derive(Debug)]
pub(super) struct Fixture {
    pub(super) core: DeviceCore,
    pub(super) pipeline: PdmaCommandPipeline,
    historical_fetches: usize,
}

pub(super) struct Prepared {
    pub(super) fixture: Fixture,
    pub(super) queue: QueueConfigHandle,
    pub(super) destinations: [MappedDmaRegion; 2],
}

impl Fixture {
    pub(super) fn new(resources: ResourceConfig) -> Self {
        Self {
            core: DeviceCore::new(
                std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
                resources,
            ),
            pipeline: PdmaCommandPipeline::new(),
            historical_fetches: 0,
        }
    }

    pub(super) fn restart(&mut self) {
        self.pipeline = PdmaCommandPipeline::new();
    }

    pub(super) const fn set_resources(&mut self, resources: ResourceConfig) {
        self.historical_fetches = self
            .historical_fetches
            .saturating_add(self.core.execution.native_steps());
        self.core.execution = crate::execution::ExecutionUnit::new(resources);
    }

    pub(super) const fn fetch_count(&self) -> usize {
        self.historical_fetches
            .saturating_add(self.core.execution.native_steps())
    }

    pub(super) fn step(&mut self) -> PipelineRunOutcome {
        normalize(&StepExecutor::new().step(&mut self.pipeline, &mut self.core))
    }

    pub(super) fn immediate(&mut self) -> PipelineRunOutcome {
        normalize(
            &ImmediateExecutor::new(NonZeroUsize::new(16).expect("nonzero budget"))
                .run(&mut self.pipeline, &mut self.core),
        )
    }
}

pub(super) fn prepare(case: Case) -> Prepared {
    prepare_with_resources(case, ResourceConfig::default())
}

pub(super) fn prepare_with_resources(case: Case, resources: ResourceConfig) -> Prepared {
    let mut fixture = Fixture::new(resources);
    fixture.core.enable_tracing();
    let source_a = fixture.core.dma.allocate(8).expect("source A");
    let source_b = fixture.core.dma.allocate(8).expect("source B");
    let destination_a = fixture.core.dma.allocate(8).expect("destination A");
    let destination_b = fixture.core.dma.allocate(8).expect("destination B");
    fixture
        .core
        .dma
        .write_object(source_a.object(), 0, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("source A bytes");
    fixture
        .core
        .dma
        .write_object(source_b.object(), 0, &[8, 7, 6, 5, 4, 3, 2, 1])
        .expect("source B bytes");
    let queue = fixture
        .core
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue");
    fixture
        .core
        .submit_command(
            queue,
            command(case, source_a, source_b, destination_a, destination_b),
        )
        .expect("submission");
    fixture.core.ring_doorbell(queue).expect("doorbell");
    let source = EpochIrqSource::new(
        queue.epoch(),
        IrqSource::Completion(queue.completion_ring_id()),
    );
    fixture.core.enable_irq_source(source).expect("enable IRQ");
    fixture.core.unmask_irq_source(source).expect("unmask IRQ");
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
    let source = prepared
        .fixture
        .core
        .dma
        .allocate(8)
        .expect("second source");
    prepared
        .fixture
        .core
        .dma
        .write_object(source.object(), 0, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("second source bytes");
    prepared
        .fixture
        .core
        .submit_command(
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

pub(super) fn drive_to_complete(fixture: &mut Fixture) -> Result<ModelStepStatus, ModelError> {
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            PipelineRunOutcome::Complete(result) => return result,
            other => panic!("unexpected step result: {other:?}"),
        }
    }
}

pub(super) fn drive_to_pending(fixture: &mut Fixture) -> PipelineRunOutcome {
    loop {
        match fixture.step() {
            PipelineRunOutcome::Progressed => {}
            outcome => return outcome,
        }
    }
}

pub(super) fn normalize(
    outcome: &Result<RunOutcome<ModelStepStatus, ModelError, AsyncStallReason>, RunError>,
) -> PipelineRunOutcome {
    match outcome {
        Ok(RunOutcome::Progressed) => PipelineRunOutcome::Progressed,
        Ok(RunOutcome::Pending(wait)) => PipelineRunOutcome::Pending(*wait),
        Ok(RunOutcome::Complete(result)) => PipelineRunOutcome::Complete(*result),
        Err(RunError::Exhausted) => PipelineRunOutcome::Exhausted,
        Err(RunError::BudgetExhausted) => PipelineRunOutcome::BudgetExhausted,
    }
}

pub(super) fn trace_kinds(
    trace: &[TraceEvent],
) -> Vec<pdma_model_contract::semantic::TraceEventKind> {
    trace.iter().map(TraceEvent::kind).collect()
}

pub(super) fn read(core: &DeviceCore, region: MappedDmaRegion) -> ModelResult<Vec<u8>> {
    core.dma.read_object(region.object(), 0, 8)
}
