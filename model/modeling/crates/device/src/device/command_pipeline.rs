mod execute;
mod fetch;
mod retire;

use std::num::NonZeroUsize;

use hardware_interface_pipeline::{
    ImmediateExecutor, PipelineExt, RunError, RunOutcome, Stage, StepExecutor, StepOutcome, Then,
};
use pdma_model_contract::semantic::{
    Command, CompletionRecord, EpochIrqSource, EventRecord, IrqSnapshot, QueueConfig,
    QueueConfigHandle, ResourceConfig, TraceEvent,
};

use self::execute::{Execute, ExecutedRoute};
use self::fetch::{FetchedRoute, ProbeFetch};
use self::retire::Retire;
use super::{DeviceCore, ModelStepStatus};
use crate::{AsyncStallReason, MappedDmaRegion, ModelError, ModelResult};

type FetchExecute = Then<ProbeFetch, fn(FetchedRoute) -> Execute, Execute>;
type CommandStages = Then<FetchExecute, fn(ExecutedRoute) -> Retire, Retire>;

#[derive(Debug)]
struct PdmaCommandPipeline {
    stages: CommandStages,
}

impl PdmaCommandPipeline {
    fn new() -> Self {
        let execute: fn(FetchedRoute) -> Execute = Execute::from;
        let retire: fn(ExecutedRoute) -> Retire = Retire::from;
        Self {
            stages: ProbeFetch::new().then(execute).then(retire),
        }
    }
}

impl Stage<DeviceCore> for PdmaCommandPipeline {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(
        &mut self,
        context: &mut DeviceCore,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.stages.step(context)
    }
}

/// Test-only direct-core facade for the private standalone pipeline.
#[doc(hidden)]
#[derive(Debug)]
pub struct PipelineFixture {
    core: DeviceCore,
    pipeline: PdmaCommandPipeline,
    historical_fetches: usize,
}

/// Publicly observable terminal state used only by [`PipelineFixture`].
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineTerminal {
    Progressed,
    FaultCached,
    Idle,
    Stalled(AsyncStallReason),
}

/// One observable executor result used only by [`PipelineFixture`].
#[doc(hidden)]
#[derive(Debug, PartialEq, Eq)]
pub enum PipelineRunOutcome {
    Progressed,
    Pending(AsyncStallReason),
    Complete(Result<PipelineTerminal, ModelError>),
    Exhausted,
    BudgetExhausted,
}

impl PipelineFixture {
    /// Creates an empty direct core with default resources.
    pub fn new() -> Self {
        Self::with_resources(ResourceConfig::default())
    }

    /// Creates an empty direct core with explicit resource limits.
    pub fn with_resources(resources: ResourceConfig) -> Self {
        Self {
            core: DeviceCore::new(
                std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
                resources,
            ),
            pipeline: PdmaCommandPipeline::new(),
            historical_fetches: 0,
        }
    }

    /// Starts a fresh accepted-command pipeline over the retained core.
    pub fn restart(&mut self) {
        self.pipeline = PdmaCommandPipeline::new();
    }

    /// Replaces execution resources while preserving fixture fetch accounting.
    pub const fn set_resources(&mut self, resources: ResourceConfig) {
        self.historical_fetches = self
            .historical_fetches
            .saturating_add(self.core.execution.native_steps());
        self.core.execution = crate::execution::ExecutionUnit::new(resources);
    }

    /// Allocates one mapped DMA object.
    pub fn allocate(&mut self, length: u64) -> Result<MappedDmaRegion, crate::PdmaMemoryError> {
        self.core.dma.allocate(length)
    }

    /// Writes mapped DMA bytes.
    pub fn write(&mut self, region: MappedDmaRegion, bytes: &[u8]) -> ModelResult<()> {
        self.core.dma.write_object(region.object(), 0, bytes)
    }

    /// Reads mapped DMA bytes.
    pub fn read(&self, region: MappedDmaRegion, length: usize) -> ModelResult<Vec<u8>> {
        self.core.dma.read_object(region.object(), 0, length)
    }

    /// Creates a semantic queue.
    pub fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.core.create_queue(config)
    }

    /// Submits one semantic command.
    pub fn submit(&mut self, queue: QueueConfigHandle, command: Command) -> ModelResult<()> {
        self.core.submit_command(queue, command)
    }

    /// Makes one queue runnable.
    pub fn doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.core.ring_doorbell(queue)
    }

    /// Polls one semantic completion.
    pub fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.core.poll_completion(queue)
    }

    /// Polls one semantic fault event.
    pub fn poll_event(&mut self, queue: QueueConfigHandle) -> ModelResult<Option<EventRecord>> {
        self.core.poll_queue_event(queue)
    }

    /// Enables trace capture.
    pub const fn enable_tracing(&mut self) {
        self.core.enable_tracing();
    }

    /// Returns captured trace records.
    pub fn trace_events(&self) -> Vec<TraceEvent> {
        self.core.trace_events()
    }

    /// Returns normalized fetch count.
    pub const fn fetch_count(&self) -> usize {
        self.historical_fetches
            .saturating_add(self.core.native_steps())
    }

    /// Enables one IRQ source.
    pub fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.core.enable_irq_source(source)
    }

    /// Unmasks one IRQ source.
    pub fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.core.unmask_irq_source(source)
    }

    /// Returns the pending PDMA IRQ snapshot.
    pub fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.core.irq_snapshot()
    }

    /// Advances the standalone pipeline once with [`StepExecutor`].
    pub fn step(&mut self) -> PipelineRunOutcome {
        let outcome = StepExecutor::new().step(&mut self.pipeline, &mut self.core);
        normalize(&outcome)
    }

    /// Runs progressed transitions with [`ImmediateExecutor`].
    pub fn run_immediate(&mut self, budget: NonZeroUsize) -> PipelineRunOutcome {
        let outcome = ImmediateExecutor::new(budget).run(&mut self.pipeline, &mut self.core);
        normalize(&outcome)
    }

    /// Advances the unchanged legacy core once.
    pub fn legacy_step(&mut self) -> Result<PipelineTerminal, ModelError> {
        self.core.step().map(PipelineTerminal::from)
    }
}

impl Default for PipelineFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ModelStepStatus> for PipelineTerminal {
    fn from(status: ModelStepStatus) -> Self {
        match status {
            ModelStepStatus::Progressed => Self::Progressed,
            ModelStepStatus::FaultCached => Self::FaultCached,
            ModelStepStatus::Idle => Self::Idle,
            ModelStepStatus::Stalled(reason) => Self::Stalled(reason),
        }
    }
}

fn normalize(
    outcome: &Result<RunOutcome<ModelStepStatus, ModelError, AsyncStallReason>, RunError>,
) -> PipelineRunOutcome {
    match outcome {
        Ok(RunOutcome::Progressed) => PipelineRunOutcome::Progressed,
        Ok(RunOutcome::Pending(wait)) => PipelineRunOutcome::Pending(*wait),
        Ok(RunOutcome::Complete(result)) => {
            PipelineRunOutcome::Complete((*result).map(PipelineTerminal::from))
        }
        Err(RunError::Exhausted) => PipelineRunOutcome::Exhausted,
        Err(RunError::BudgetExhausted) => PipelineRunOutcome::BudgetExhausted,
    }
}
