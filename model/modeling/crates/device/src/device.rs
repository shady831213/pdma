//! PDMA device model with command queue, doorbell, scheduler, engine, and completion path.

mod progress;
mod retirement;

use crate::arbitration::{ArbitrationPolicy, QueueArbiter};
use crate::memory::Memory;
use crate::resources::ResourcePool;
use crate::rings::Ring;
use crate::trace::TraceSink;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle, QueueDirectoryEntry};
use pdma_model_contract::resources::ResourceConfig;
use pdma_model_contract::trace::TraceEvent;
pub use progress::{RunStatusSummary, RunSummary, StallReason, StepStatus};
use retirement::PendingRetirement;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
struct QueueContext {
    entry: QueueDirectoryEntry,
    command_ring: Ring<ExternalCommand>,
}

/// Top-level deterministic PDMA device model.
#[derive(Debug, Clone)]
pub struct PdmaDevice {
    memory: Memory,
    queue_directory: Vec<QueueDirectoryEntry>,
    queues: Vec<QueueContext>,
    completions: Vec<Ring<CompletionRecord>>,
    event_ring: VecDeque<EventRecord>,
    runnable: Vec<QueueId>,
    arbiter: QueueArbiter,
    resources: ResourcePool,
    pending_retirement: Option<PendingRetirement>,
    trace: TraceSink,
}

impl PdmaDevice {
    /// Creates an empty device.
    pub fn new() -> Self {
        Self {
            memory: Memory::default(),
            queue_directory: Vec::new(),
            queues: Vec::new(),
            completions: Vec::new(),
            event_ring: VecDeque::new(),
            runnable: Vec::new(),
            arbiter: QueueArbiter::default(),
            resources: ResourcePool::new(ResourceConfig::default()),
            pending_retirement: None,
            trace: TraceSink::default(),
        }
    }

    /// Creates an empty device with the requested execution resource capacities.
    pub fn with_resource_config(config: ResourceConfig) -> Self {
        Self {
            resources: ResourcePool::new(config),
            ..Self::new()
        }
    }

    /// Creates an empty device using the requested arbitration policy.
    pub fn with_arbitration_policy(policy: ArbitrationPolicy) -> Self {
        Self {
            arbiter: QueueArbiter::new(policy),
            ..Self::new()
        }
    }

    /// Returns the active queue arbitration policy.
    pub const fn arbitration_policy(&self) -> ArbitrationPolicy {
        self.arbiter.policy()
    }

    /// Enables deterministic trace event capture.
    pub const fn enable_tracing(&mut self) -> &mut TraceSink {
        self.trace.enable()
    }

    /// Disables deterministic trace event capture.
    pub const fn disable_tracing(&mut self) {
        self.trace.disable();
    }

    /// Clears all captured trace events.
    pub fn clear_trace(&mut self) {
        self.trace.clear();
    }

    /// Returns captured trace events in deterministic sequence order.
    pub fn trace_events(&self) -> &[TraceEvent] {
        self.trace.events()
    }

    /// Exports captured trace events as minimal Chrome/Perfetto JSON.
    pub fn trace_json(&self) -> String {
        self.trace.to_json()
    }

    /// Allocates a device-visible memory buffer.
    pub fn allocate_buffer(&mut self, len: usize) -> BufferId {
        self.memory.allocate(len)
    }

    /// Writes bytes into a checked device buffer range.
    pub fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.memory.write_checked(id, offset, data)
    }

    /// Reads bytes from a checked device buffer range.
    pub fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        self.memory.read_checked(id, offset, len)
    }

    /// Provisions one command queue and one bound completion ring.
    pub fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        let command_ring = Ring::with_capacity(config.command_depth())?;
        let completion_ring = Ring::with_capacity(config.completion_depth())?;
        let queue_id = QueueId::new(self.queue_directory.len());
        let completion_id = CompletionRingId::new(self.completions.len());
        let entry = QueueDirectoryEntry::new(queue_id, completion_id, config);
        self.queue_directory.push(entry);
        self.completions.push(completion_ring);
        self.queues.push(QueueContext {
            entry,
            command_ring,
        });
        Ok(QueueConfigHandle::new(entry))
    }

    /// Returns one queue directory entry.
    pub fn queue_directory_entry(&self, queue_id: QueueId) -> ModelResult<QueueDirectoryEntry> {
        self.queue(queue_id).map(|queue| queue.entry)
    }

    /// Pushes a command into a queue's command ring.
    pub fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        let operation = crate::trace::operation_from_external(&command);
        let command_id = crate::trace::command_id(&command);
        self.queue_mut(queue_id)?.command_ring.push(command)?;
        self.trace
            .command_submitted(queue_id, operation, command_id);
        Ok(())
    }

    /// Captures a software doorbell and marks the queue runnable.
    pub fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        self.queue(queue_id)?;
        if !self.runnable.contains(&queue_id) {
            self.runnable.push(queue_id);
        }
        self.trace.doorbell_runnable(queue_id);
        Ok(())
    }

    /// Pops one completion record for software polling.
    pub fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        let completion = self.completion_mut(completion_ring_id)?.pop();
        if let Some(record) = completion {
            self.trace.completion_polled(record);
        }
        Ok(completion)
    }

    /// Pops one event record for software polling.
    pub fn poll_event(&mut self) -> ModelResult<Option<EventRecord>> {
        let event = self.event_ring.pop_front();
        self.trace.event_poll(None, event);
        Ok(event)
    }

    /// Pops one event record for a specific queue without consuming other queues' events.
    pub fn poll_queue_event(&mut self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        self.queue(queue_id)?;
        let Some(index) = self
            .event_ring
            .iter()
            .position(|event| event.queue_id() == queue_id)
        else {
            self.trace.event_poll(Some(queue_id), None);
            return Ok(None);
        };
        let event = self.event_ring.remove(index);
        self.trace.event_poll(Some(queue_id), event);
        Ok(event)
    }

    fn queue(&self, id: QueueId) -> ModelResult<&QueueContext> {
        self.queues
            .iter()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    fn queue_mut(&mut self, id: QueueId) -> ModelResult<&mut QueueContext> {
        self.queues
            .iter_mut()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    fn completion_mut(&mut self, id: CompletionRingId) -> ModelResult<&mut Ring<CompletionRecord>> {
        self.completions
            .get_mut(id.raw())
            .ok_or(ModelError::UnknownCompletionRing(id))
    }
}

impl Default for PdmaDevice {
    fn default() -> Self {
        Self::new()
    }
}
