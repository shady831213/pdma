use super::io::{FaultPublication, NormalizedFetch, PublicationOutcome};
use super::{PdmaQueue, QueueBlock};
use crate::dma::PdmaMemoryMappings;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    Command, CommandId, CompletionRecord, CompletionRingId, DeviceEpoch, EventRecord, QueueConfig,
    QueueConfigHandle, QueueDirectoryEntry, QueueId, TraceOperation,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SubmittedCommand {
    pub(crate) queue: QueueId,
    pub(crate) command: CommandId,
    pub(crate) operation: TraceOperation,
}

impl QueueBlock {
    pub(crate) fn create_semantic(
        &mut self,
        epoch: DeviceEpoch,
        config: QueueConfig,
    ) -> ModelResult<QueueConfigHandle> {
        let queue_id = QueueId::new(self.contexts.len());
        let completion_id = CompletionRingId::new(self.contexts.len());
        let queue = PdmaQueue::semantic(queue_id, completion_id, config)?;
        let entry = queue.entry;
        self.contexts.push(queue);
        Ok(QueueConfigHandle::new(epoch, entry))
    }

    pub(crate) fn submit_semantic(
        &mut self,
        queue: QueueId,
        command: Command,
    ) -> ModelResult<SubmittedCommand> {
        self.get_mut(queue)?
            .io
            .submit_semantic(queue, command)
            .map(|(command, operation)| SubmittedCommand {
                queue,
                command,
                operation,
            })
    }

    pub(crate) fn poll_semantic_completion(
        &mut self,
        completion: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        Ok(self
            .get_by_completion_mut(completion)?
            .io
            .poll_semantic_completion())
    }

    pub(crate) fn poll_semantic_event(
        &mut self,
        queue: QueueId,
    ) -> ModelResult<Option<EventRecord>> {
        Ok(self.get_mut(queue)?.io.poll_semantic_event(queue))
    }

    pub(crate) fn next_queue_id(&self) -> QueueId {
        QueueId::new(self.contexts.len())
    }

    pub(crate) fn entry(&self, queue: QueueId) -> ModelResult<QueueDirectoryEntry> {
        self.get(queue).map(|context| context.entry)
    }

    #[cfg(test)]
    pub(crate) fn semantic_cursors(
        &self,
        queue: QueueId,
    ) -> ModelResult<((usize, usize), (usize, usize))> {
        self.get(queue)?
            .io
            .semantic_cursors()
            .ok_or(ModelError::UnknownQueue(queue))
    }

    pub(crate) fn contains_completion(&self, completion: CompletionRingId) -> bool {
        self.get_by_completion(completion).is_ok()
    }

    pub(crate) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<FaultPublication> {
        Ok(self.get_mut(queue)?.io.publish_fault(memory))
    }

    pub(crate) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
        record: CompletionRecord,
    ) -> ModelResult<PublicationOutcome> {
        Ok(self.get_mut(queue)?.io.publish_completion(memory, record))
    }

    pub(crate) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<Option<NormalizedFetch>> {
        let context = self.get_mut(queue)?;
        context.io.fetch(memory, context.entry)
    }

    pub(crate) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<Option<NormalizedFetch>> {
        let context = self.get(queue)?;
        context.io.probe(memory, context.entry)
    }

    pub(crate) fn consume(&mut self, queue: QueueId) -> ModelResult<()> {
        self.get_mut(queue)?.io.consume()
    }

    pub(crate) fn has_pending(&self, queue: QueueId) -> ModelResult<bool> {
        Ok(self.get(queue)?.io.has_pending())
    }

    pub(crate) fn has_any_pending(&self) -> bool {
        self.contexts.iter().any(|queue| queue.io.has_pending())
    }

    pub(crate) fn stage_execution_fault(
        &mut self,
        queue: QueueId,
        event: EventRecord,
    ) -> ModelResult<ModelError> {
        self.get_mut(queue)?.io.stage_execution_fault(event)
    }

    pub(crate) fn completion_blocked(
        &self,
        queue: QueueId,
        publication_slots: usize,
    ) -> ModelResult<bool> {
        Ok(self.get(queue)?.io.completion_blocked(publication_slots))
    }

    pub(crate) fn fault_blocked(
        &self,
        queue: QueueId,
        publication_slots: usize,
    ) -> ModelResult<bool> {
        Ok(self.get(queue)?.io.fault_blocked(publication_slots))
    }

    pub(crate) fn records_pending(&self, queue: QueueId, completion: bool) -> ModelResult<bool> {
        self.get(queue)?.io.records_pending(completion)
    }

    pub(crate) fn completion_records_pending(
        &self,
        completion: CompletionRingId,
    ) -> ModelResult<bool> {
        self.get_by_completion(completion)?.io.records_pending(true)
    }

    pub(crate) fn pending_exact_fault(
        &self,
        queue: QueueId,
    ) -> Option<super::exact::PendingDmaResult> {
        self.get(queue)
            .ok()
            .and_then(|context| context.io.pending_exact_fault())
    }

    pub(crate) fn clear(&mut self) {
        self.contexts.clear();
    }

    pub(crate) fn get(&self, id: QueueId) -> ModelResult<&PdmaQueue> {
        self.contexts
            .iter()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    pub(crate) fn get_mut(&mut self, id: QueueId) -> ModelResult<&mut PdmaQueue> {
        self.contexts
            .iter_mut()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    pub(crate) fn get_by_completion(&self, id: CompletionRingId) -> ModelResult<&PdmaQueue> {
        self.contexts
            .iter()
            .find(|queue| queue.entry.completion_ring_id() == id)
            .ok_or(ModelError::UnknownCompletionRing(id))
    }

    fn get_by_completion_mut(&mut self, id: CompletionRingId) -> ModelResult<&mut PdmaQueue> {
        self.contexts
            .iter_mut()
            .find(|queue| queue.entry.completion_ring_id() == id)
            .ok_or(ModelError::UnknownCompletionRing(id))
    }
}
