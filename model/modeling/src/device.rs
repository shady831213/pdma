//! PDMA device model with command queue, doorbell, scheduler, engine, and completion path.

use crate::completion::{CompletionRecord, CompletionStatus};
use crate::desc::ExternalCommand;
use crate::engines::{CopyEngine, EngineError};
use crate::event::EventRecord;
use crate::ids::{CompletionRingId, QueueId};
use crate::memory::Memory;
use crate::rings::Ring;
use crate::scheduler::Scheduler;
use crate::{ModelError, ModelResult};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
struct QueueContext {
    id: QueueId,
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
    queues: Vec<QueueContext>,
    completions: Vec<Ring<CompletionRecord>>,
    event_ring: VecDeque<EventRecord>,
    runnable: Vec<QueueId>,
}

impl PdmaDevice {
    /// Creates an empty device.
    pub fn new() -> Self {
        Self {
            memory: Memory::default(),
            queues: Vec::new(),
            completions: Vec::new(),
            event_ring: VecDeque::new(),
            runnable: Vec::new(),
        }
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
    pub fn create_queue(
        &mut self,
        command_depth: usize,
        completion_depth: usize,
    ) -> ModelResult<(QueueId, CompletionRingId)> {
        let queue_id = QueueId::new(self.queues.len());
        let completion_id = CompletionRingId::new(self.completions.len());
        self.completions
            .push(Ring::with_capacity(completion_depth)?);
        self.queues.push(QueueContext {
            id: queue_id,
            command_ring: Ring::with_capacity(command_depth)?,
        });
        Ok((queue_id, completion_id))
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
        let Some(queue_id) = self.runnable.pop() else {
            return Ok(false);
        };
        let command = self.queue_mut(queue_id)?.command_ring.pop();
        let Some(command) = command else {
            return Ok(true);
        };
        let has_more_commands = !self.queue(queue_id)?.command_ring.is_empty();
        let result = (|| {
            let work = Scheduler::lower(command)?;
            if let Err(error) = CopyEngine::execute(&mut self.memory, &work) {
                match error {
                    EngineError::Memory(fault) => {
                        let event = EventRecord::new(
                            work.queue_id(),
                            work.command_id(),
                            fault.error_kind(),
                            fault.buffer(),
                        );
                        self.event_ring.push_back(event);
                        return Err(fault.source());
                    }
                    EngineError::Model(error) => return Err(error),
                }
            }
            let completion = CompletionRecord::new(
                work.queue_id(),
                work.command_id(),
                CompletionStatus::Success,
            );
            self.completion_mut(work.completion_ring_id())?
                .push(completion)
        })();
        if has_more_commands {
            self.runnable.push(queue_id);
        }
        result?;
        Ok(true)
    }

    /// Runs deterministic steps until idle or until the step budget expires.
    pub fn run_until_idle(&mut self, max_steps: usize) -> ModelResult<RunSummary> {
        let mut steps = 0;
        while steps < max_steps {
            if !self.step()? {
                return Ok(RunSummary { steps });
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
        !self.runnable.is_empty()
            || self
                .queues
                .iter()
                .any(|queue| !queue.command_ring.is_empty())
    }

    fn queue(&self, id: QueueId) -> ModelResult<&QueueContext> {
        self.queues
            .iter()
            .find(|queue| queue.id == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    fn queue_mut(&mut self, id: QueueId) -> ModelResult<&mut QueueContext> {
        self.queues
            .iter_mut()
            .find(|queue| queue.id == id)
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
