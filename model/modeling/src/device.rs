//! PDMA device model with command queue, doorbell, scheduler, engine, and completion path.

mod retirement;

use crate::arbitration::{ArbitrationPolicy, QueueArbiter};
use crate::completion::CompletionRecord;
use crate::desc::ExternalCommand;
use crate::event::EventRecord;
use crate::ids::{CompletionRingId, QueueId};
use crate::memory::Memory;
use crate::queue::{QueueConfig, QueueConfigHandle, QueueDirectoryEntry};
use crate::resources::{ResourceConfig, ResourceDemand, ResourcePool};
use crate::rings::Ring;
use crate::scheduler::Scheduler;
use crate::{ModelError, ModelResult};
use retirement::PendingRetirement;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
struct QueueContext {
    entry: QueueDirectoryEntry,
    command_ring: Ring<ExternalCommand>,
}

/// Summary returned by `run_until_idle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSummary {
    /// Number of device steps that changed state.
    pub steps: usize,
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

    /// Allocates a device-visible memory buffer.
    pub fn allocate_buffer(&mut self, len: usize) -> crate::ids::BufferId {
        self.memory.allocate(len)
    }

    /// Writes bytes into a checked device buffer range.
    pub fn write_buffer(
        &mut self,
        id: crate::ids::BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        self.memory.write_checked(id, offset, data)
    }

    /// Reads bytes from a checked device buffer range.
    pub fn read_buffer(
        &self,
        id: crate::ids::BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
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
        self.queue_mut(queue_id)?.command_ring.push(command)
    }

    /// Captures a software doorbell and marks the queue runnable.
    pub fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        self.queue(queue_id)?;
        if !self.runnable.contains(&queue_id) {
            self.runnable.push(queue_id);
        }
        Ok(())
    }

    /// Advances the deterministic device model by at most one command.
    pub fn step(&mut self) -> ModelResult<bool> {
        if let Some(pending) = self.pending_retirement {
            return self.retry_pending_retirement(pending);
        }
        let Some(queue_id) = self.arbiter.select_next(&mut self.runnable) else {
            return Ok(false);
        };
        let Some(command) = self.queue(queue_id)?.command_ring.front() else {
            return Ok(true);
        };
        let work = Scheduler::lower(command.clone())?;
        let demand = ResourceDemand::for_work(&work);
        if !self.resources.has_available(demand) {
            self.runnable.push(queue_id);
            return Ok(false);
        }
        self.resources.acquire(demand);
        let result = self.retire_ready_work(queue_id, &work);
        self.resources.release(demand);
        result
    }

    /// Runs deterministic steps until idle or until the step budget expires.
    pub fn run_until_idle(&mut self, max_steps: usize) -> ModelResult<RunSummary> {
        let mut steps = 0;
        while steps < max_steps {
            if !self.step()? {
                return if self.has_work() {
                    Err(ModelError::StepBudgetExhausted { max_steps })
                } else {
                    Ok(RunSummary { steps })
                };
            }
            steps += 1;
        }
        if self.has_work() {
            Err(ModelError::StepBudgetExhausted { max_steps })
        } else {
            Ok(RunSummary { steps })
        }
    }

    /// Pops one completion record for software polling.
    pub fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        Ok(self.completion_mut(completion_ring_id)?.pop())
    }

    /// Pops one event record for software polling.
    pub fn poll_event(&mut self) -> ModelResult<Option<EventRecord>> {
        Ok(self.event_ring.pop_front())
    }

    /// Pops one event record for a specific queue without consuming other queues' events.
    pub fn poll_queue_event(&mut self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        self.queue(queue_id)?;
        let Some(index) = self
            .event_ring
            .iter()
            .position(|event| event.queue_id() == queue_id)
        else {
            return Ok(None);
        };
        Ok(self.event_ring.remove(index))
    }

    fn has_work(&self) -> bool {
        self.pending_retirement.is_some()
            || !self.runnable.is_empty()
            || self
                .queues
                .iter()
                .any(|queue| !queue.command_ring.is_empty())
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
