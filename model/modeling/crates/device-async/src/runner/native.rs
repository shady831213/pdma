mod broadcast;
mod copy;
mod memory;
mod ring;
mod step;

use super::backend::BackendStepStatus;
use memory::NativeMemory;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle, QueueDirectoryEntry};
use ring::NativeRing;
use std::collections::VecDeque;

#[derive(Debug)]
struct QueueContext {
    entry: QueueDirectoryEntry,
    commands: NativeRing<ExternalCommand>,
}

#[derive(Debug, Default)]
pub(super) struct NativeDevice {
    memory: NativeMemory,
    queues: Vec<QueueContext>,
    completions: Vec<NativeRing<CompletionRecord>>,
    events: VecDeque<EventRecord>,
    runnable: VecDeque<QueueId>,
    pending_completion: Option<copy::PendingCompletion>,
}

impl NativeDevice {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        let commands = NativeRing::with_capacity(config.command_depth())?;
        let completions = NativeRing::with_capacity(config.completion_depth())?;
        let queue_id = QueueId::new(self.queues.len());
        let completion_id = CompletionRingId::new(self.completions.len());
        let entry = QueueDirectoryEntry::new(queue_id, completion_id, config);
        self.queues.push(QueueContext { entry, commands });
        self.completions.push(completions);
        Ok(QueueConfigHandle::new(entry))
    }

    pub(super) fn allocate_buffer(&mut self, len: usize) -> BufferId {
        self.memory.allocate(len)
    }

    pub(super) fn write_buffer(
        &mut self,
        id: BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        self.memory.write_checked(id, offset, data)
    }

    pub(super) fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        self.memory.read_checked(id, offset, len)
    }

    pub(super) fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.queue_mut(queue_id)?.commands.push(command)
    }

    pub(super) fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        self.queue(queue_id)?;
        if !self.runnable.contains(&queue_id) {
            self.runnable.push_back(queue_id);
        }
        Ok(())
    }

    pub(super) fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.completion_mut(completion_ring_id).map(NativeRing::pop)
    }

    pub(super) fn poll_queue_event(
        &mut self,
        queue_id: QueueId,
    ) -> ModelResult<Option<EventRecord>> {
        self.queue(queue_id)?;
        let Some(index) = self
            .events
            .iter()
            .position(|event| event.queue_id() == queue_id)
        else {
            return Ok(None);
        };
        Ok(self.events.remove(index))
    }

    pub(super) fn step(&mut self) -> ModelResult<BackendStepStatus> {
        step::step(self)
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

    fn completion_mut(
        &mut self,
        id: CompletionRingId,
    ) -> ModelResult<&mut NativeRing<CompletionRecord>> {
        self.completions
            .get_mut(id.raw())
            .ok_or(ModelError::UnknownCompletionRing(id))
    }
}
