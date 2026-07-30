use super::backend::{DeviceModel, ModelStepStatus};
use super::bus::control::BusControlState;
use super::{AsyncRunSummary, AsyncStepStatus};
use crate::dma_space::{DmaSpace, DmaSpaceError, DmaStage};
use pdma_model_contract::ResourceConfig;
use pdma_model_contract::abi::EpochIrqSource;
use pdma_model_contract::abi::{DmaAddress, DmaRegion};
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::BufferId;
use pdma_model_contract::irq::{IrqNotification, IrqRoute, IrqSnapshot};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::trace::TraceEvent;

/// Synchronous device surface for deterministic bounded steps without a worker task.
#[derive(Debug)]
pub struct SyncDevice {
    pub(in crate::runner) backend: DeviceModel,
    dma_space: DmaSpace,
    bus_control: BusControlState,
    steps: usize,
    last_status: AsyncStepStatus,
}

impl SyncDevice {
    /// Creates a synchronous device with default resource credits.
    pub fn new() -> Self {
        Self::with_resource_config(ResourceConfig::default())
    }

    /// Creates a synchronous device with explicit resource credits.
    pub fn with_resource_config(config: ResourceConfig) -> Self {
        Self {
            backend: DeviceModel::queue_port_with_resource_config(config),
            dma_space: DmaSpace::new(),
            bus_control: BusControlState::new(),
            steps: 0,
            last_status: AsyncStepStatus::Idle,
        }
    }

    /// Provisions one command queue and one bound completion ring.
    pub fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.backend.create_queue(config)
    }

    /// Provisions a model-only queue using the decoded DMA-bus frontend seam.
    pub fn create_dma_bus_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.backend.create_dma_bus_queue(config)
    }

    /// Allocates a device-visible memory buffer.
    pub fn allocate_buffer(&mut self, len: usize) -> ModelResult<BufferId> {
        self.allocate_dma_region(len).map(DmaRegion::object)
    }

    fn allocate_dma_region(&mut self, len: usize) -> ModelResult<DmaRegion> {
        let length = u64::try_from(len).map_err(|_| ModelError::DmaAllocationExhausted)?;
        self.dma_space
            .allocate(length)
            .map_err(|_| ModelError::DmaAllocationExhausted)
    }

    /// Writes bytes into a checked device buffer range.
    pub fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.dma_space.write_object(id, offset, data)
    }

    /// Reads bytes from a checked device buffer range.
    pub fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        self.dma_space.read_object(id, offset, len)
    }

    /// Proposes DMA regions without changing mappings, bytes, or allocator generation.
    pub fn stage_dma_regions(&self, lengths: &[u64]) -> Result<DmaStage, DmaSpaceError> {
        self.dma_space.stage(lengths)
    }

    /// Atomically commits every region in a stage.
    pub fn commit_dma_stage(&mut self, stage: DmaStage) -> Result<Vec<DmaRegion>, DmaSpaceError> {
        self.dma_space.commit(stage)
    }

    /// Reads one checked IOVA extent.
    pub fn read_iova(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, DmaSpaceError> {
        self.dma_space.read_iova(address, length)
    }

    /// Writes one checked IOVA extent.
    pub fn write_iova(&mut self, address: DmaAddress, data: &[u8]) -> Result<(), DmaSpaceError> {
        self.dma_space.write_iova(address, data)
    }

    /// Resolves a semantic object to its committed DMA region.
    pub fn region_for_object(&self, object: BufferId) -> Option<DmaRegion> {
        self.dma_space.region_for_object(object)
    }

    /// Returns the committed DMA allocator generation.
    pub const fn dma_generation(&self) -> u64 {
        self.dma_space.generation()
    }

    /// Pushes one command into a queue's command ring.
    pub fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.backend.submit_command(queue, command)
    }

    /// Captures a software doorbell for one queue.
    pub fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.backend.ring_doorbell(queue)
    }

    /// Pops one completion from a completion ring.
    pub fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.backend.poll_completion(queue)
    }

    /// Pops one event for a queue-local event path.
    pub fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.backend.poll_queue_event(queue)
    }

    /// Enables a PDMA-local IRQ source.
    pub fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.backend.enable_irq_source(source)
    }

    /// Configures the delivery route of an inactive PDMA-local IRQ source.
    pub fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.backend.configure_irq_route(source, route)
    }

    /// Disables a PDMA-local IRQ source.
    pub fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.backend.disable_irq_source(source)
    }

    /// Masks a PDMA-local IRQ source without clearing pending state.
    pub fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.backend.mask_irq_source(source)
    }

    /// Unmasks a PDMA-local IRQ source.
    pub fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.backend.unmask_irq_source(source)
    }

    /// Acknowledges a source, retaining a Line cause while source records remain.
    pub fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.backend.ack_irq_source(&self.bus_control, source)
    }

    /// Returns the current pending IRQ snapshot, if any.
    pub fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.backend.irq_snapshot()
    }

    /// Returns the currently asserted IRQ snapshot, if any.
    pub fn wait_irq(&self) -> Option<IrqSnapshot> {
        self.irq_snapshot().filter(|snapshot| snapshot.asserted())
    }

    /// Returns and consumes one deliverable line or message IRQ notification.
    pub fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.backend.take_irq_notification()
    }

    /// Runs deterministic synchronous steps until idle or until the step budget expires.
    pub fn run_until_idle(&mut self, max_steps: usize) -> ModelResult<AsyncRunSummary> {
        let start_steps = self.steps;
        while self.steps - start_steps < max_steps {
            match self
                .backend
                .step(&mut self.dma_space, &mut self.bus_control)?
            {
                ModelStepStatus::Progressed => {
                    self.steps = self.steps.saturating_add(1);
                    self.last_status = AsyncStepStatus::Progressed;
                }
                ModelStepStatus::Idle => {
                    self.last_status = AsyncStepStatus::Idle;
                    return self.idle_or_budget_exhausted(max_steps, start_steps);
                }
                ModelStepStatus::Stalled(reason) => {
                    self.last_status = AsyncStepStatus::Stalled(reason);
                    return self.idle_or_budget_exhausted(max_steps, start_steps);
                }
            }
        }
        self.idle_or_budget_exhausted(max_steps, start_steps)
    }

    /// Runs deterministic synchronous steps until idle, stalled, or budget exhaustion.
    pub fn run_until_blocked(&mut self, max_steps: usize) -> ModelResult<AsyncRunSummary> {
        let start_steps = self.steps;
        let mut stalls = 0;
        while self.steps - start_steps < max_steps {
            match self
                .backend
                .step(&mut self.dma_space, &mut self.bus_control)?
            {
                ModelStepStatus::Progressed => {
                    self.steps = self.steps.saturating_add(1);
                    self.last_status = AsyncStepStatus::Progressed;
                }
                ModelStepStatus::Idle => {
                    self.last_status = AsyncStepStatus::Idle;
                    return Ok(AsyncRunSummary {
                        steps: self.steps - start_steps,
                        errors: 0,
                        stalls,
                        last_status: self.last_status,
                    });
                }
                ModelStepStatus::Stalled(reason) => {
                    stalls += 1;
                    self.last_status = AsyncStepStatus::Stalled(reason);
                    return Ok(AsyncRunSummary {
                        steps: self.steps - start_steps,
                        errors: 0,
                        stalls,
                        last_status: self.last_status,
                    });
                }
            }
        }
        if self.backend.has_work(&self.dma_space, &self.bus_control) {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(AsyncRunSummary {
                steps: self.steps - start_steps,
                errors: 0,
                stalls,
                last_status: AsyncStepStatus::Idle,
            })
        }
    }

    /// Enables deterministic trace capture.
    pub const fn enable_tracing(&mut self) {
        self.backend.enable_tracing();
    }

    /// Disables trace capture without clearing existing events.
    pub const fn disable_tracing(&mut self) {
        self.backend.disable_tracing();
    }

    /// Clears captured trace events and sequence ordinals.
    pub fn clear_trace(&mut self) {
        self.backend.clear_trace();
    }

    /// Returns captured trace events in deterministic sequence order.
    pub fn trace_events(&self) -> Vec<TraceEvent> {
        self.backend.trace_events()
    }

    /// Exports captured trace events as Chrome/Perfetto JSON.
    pub fn trace_json(&self) -> String {
        self.backend.trace_json()
    }

    fn idle_or_budget_exhausted(
        &self,
        max_steps: usize,
        start_steps: usize,
    ) -> ModelResult<AsyncRunSummary> {
        if self.backend.has_work(&self.dma_space, &self.bus_control) {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(AsyncRunSummary {
                steps: self.steps - start_steps,
                errors: 0,
                stalls: 0,
                last_status: self.last_status,
            })
        }
    }
}

impl Default for SyncDevice {
    fn default() -> Self {
        Self::new()
    }
}
