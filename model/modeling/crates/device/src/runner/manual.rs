use super::DeviceCore;
use crate::dma_space::{PdmaMemoryError, PdmaMemoryStage};
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::bridge::PdmaMemoryRegion;
use pdma_model_contract::semantic::EpochIrqSource;
use pdma_model_contract::semantic::completion::CompletionRecord;
use pdma_model_contract::semantic::desc::ExternalCommand;
use pdma_model_contract::semantic::event::EventRecord;
use pdma_model_contract::semantic::ids::BufferId;
use pdma_model_contract::semantic::irq::{IrqNotification, IrqRoute, IrqSnapshot};
use pdma_model_contract::semantic::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::semantic::trace::TraceEvent;
use std::ops::{Deref, DerefMut};

mod run;

/// Synchronous device surface for deterministic bounded steps without a worker task.
#[derive(Debug)]
pub struct SyncDevice {
    core: DeviceCore,
}

impl SyncDevice {
    /// Consumes one device core into the synchronous compatibility facade.
    pub const fn from_core(core: DeviceCore) -> Self {
        Self { core }
    }

    pub(crate) fn reset_core(
        &mut self,
        request: pdma_model_contract::semantic::WholeDeviceReset,
    ) -> Result<pdma_model_contract::semantic::DeviceEpoch, super::bus::PdmaControlError> {
        self.core.reset_runner(request)
    }

    /// Returns committed invocations of the sole reset runner.
    pub const fn reset_runner_calls(&self) -> usize {
        self.core.reset_runner_calls()
    }

    /// Returns the persistent resource configuration owned by the core.
    pub const fn resource_config(&self) -> pdma_model_contract::semantic::ResourceConfig {
        self.core.backend.resource_config()
    }

    /// Returns persistent lifecycle step accounting owned by the core.
    pub const fn lifecycle_steps(&self) -> usize {
        self.core.steps
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
        self.allocate_memory_region(len)
            .map(PdmaMemoryRegion::object)
    }

    /// Allocates one logical region through the shared core mapping adapter.
    pub fn allocate_memory_region(&mut self, len: usize) -> ModelResult<PdmaMemoryRegion> {
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
    pub fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        self.dma_space.stage(lengths)
    }

    /// Atomically commits every region in a stage.
    pub fn commit_memory_stage(
        &mut self,
        stage: PdmaMemoryStage,
    ) -> Result<Vec<PdmaMemoryRegion>, PdmaMemoryError> {
        self.dma_space.commit(stage)
    }

    /// Reads one checked mapped-memory extent.
    pub fn read_memory(
        &self,
        address: impl Into<MemoryAddress>,
        length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        self.dma_space.read_memory(address, length)
    }

    /// Writes one checked mapped-memory extent.
    pub fn write_memory(
        &mut self,
        address: impl Into<MemoryAddress>,
        data: &[u8],
    ) -> Result<(), PdmaMemoryError> {
        self.dma_space.write_memory(address, data)
    }

    /// Resolves a semantic object to its committed DMA region.
    pub fn region_for_object(&self, object: BufferId) -> Option<PdmaMemoryRegion> {
        self.dma_space.region_for_object(object)
    }

    /// Returns the committed DMA allocator generation.
    pub const fn memory_generation(&self) -> u64 {
        self.core.dma_space.generation()
    }

    /// Returns the number of logical PDMA mappings.
    pub fn mapped_region_count(&self) -> usize {
        self.core.dma_space.mapped_region_count()
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
        self.core
            .backend
            .ack_irq_source(&self.core.bus_control, source)
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

    /// Enables deterministic trace capture.
    pub const fn enable_tracing(&mut self) {
        self.core.backend.enable_tracing();
    }

    /// Disables trace capture without clearing existing events.
    pub const fn disable_tracing(&mut self) {
        self.core.backend.disable_tracing();
    }

    /// Clears captured trace events and sequence ordinals.
    pub fn clear_trace(&mut self) {
        self.backend.clear_trace();
    }

    /// Returns captured trace events in deterministic sequence order.
    pub fn trace_events(&self) -> Vec<TraceEvent> {
        self.backend.trace_events()
    }

    /// Returns normalized frontend fetches that entered the native step core.
    pub fn native_steps(&self) -> usize {
        self.backend.native_steps()
    }

    /// Exports captured trace events as Chrome/Perfetto JSON.
    pub fn trace_json(&self) -> String {
        self.backend.trace_json()
    }
}

impl Deref for SyncDevice {
    type Target = DeviceCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for SyncDevice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}
