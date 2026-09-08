use super::io::{FaultPublication, NormalizedFetch, PublicationOutcome};
use super::{ExactQueueIo, QueueBlock};
use crate::memory::PdmaMemoryMappings;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{AbiCompletion, AbiEvent};
use pdma_model_contract::semantic::{CompletionRingId, QueueId};

impl QueueBlock {
    pub(crate) fn contains_completion(&self, completion: CompletionRingId) -> bool {
        self.get(QueueId::new(completion.raw())).is_ok()
    }

    pub(crate) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<FaultPublication> {
        Ok(self.get_mut(queue)?.publish_fault(memory))
    }

    pub(crate) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
        record: AbiCompletion,
    ) -> ModelResult<PublicationOutcome> {
        Ok(self.get_mut(queue)?.publish_completion(memory, record))
    }

    pub(crate) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<Option<NormalizedFetch>> {
        self.get_mut(queue)?.fetch(memory, queue)
    }

    pub(crate) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        queue: QueueId,
    ) -> ModelResult<Option<NormalizedFetch>> {
        self.get(queue)?.probe(memory, queue)
    }

    pub(crate) fn consume(&mut self, queue: QueueId) -> ModelResult<()> {
        self.get_mut(queue)?.consume()
    }

    pub(crate) fn has_pending(&self, queue: QueueId) -> ModelResult<bool> {
        Ok(self.get(queue)?.has_pending())
    }

    pub(crate) fn has_any_pending(&self) -> bool {
        self.contexts
            .iter()
            .filter_map(|queue| queue.io.as_ref())
            .any(ExactQueueIo::has_pending)
    }

    pub(crate) fn stage_execution_fault(
        &mut self,
        queue: QueueId,
        event: AbiEvent,
    ) -> ModelResult<()> {
        self.get_mut(queue)?.stage_execution_fault(event);
        Ok(())
    }

    pub(crate) fn completion_blocked(
        &self,
        queue: QueueId,
        publication_slots: usize,
    ) -> ModelResult<bool> {
        Ok(self.get(queue)?.completion_blocked(publication_slots))
    }

    pub(crate) fn fault_blocked(
        &self,
        queue: QueueId,
        publication_slots: usize,
    ) -> ModelResult<bool> {
        Ok(self.get(queue)?.fault_blocked(publication_slots))
    }

    pub(crate) fn records_pending(&self, queue: QueueId, completion: bool) -> ModelResult<bool> {
        Ok(self.get(queue)?.records_pending(completion))
    }

    pub(crate) fn completion_records_pending(
        &self,
        completion: CompletionRingId,
    ) -> ModelResult<bool> {
        Ok(self
            .get(QueueId::new(completion.raw()))?
            .records_pending(true))
    }

    pub(crate) fn pending_exact_fault(
        &self,
        queue: QueueId,
    ) -> Option<super::exact::PendingDmaResult> {
        self.get(queue).ok().and_then(ExactQueueIo::pending_result)
    }

    pub(crate) fn clear(&mut self) {
        self.contexts.clear();
    }

    pub(super) fn get(&self, id: QueueId) -> ModelResult<&ExactQueueIo> {
        self.contexts
            .get(id.raw())
            .and_then(|queue| queue.io.as_ref())
            .ok_or(ModelError::UnknownQueue(id))
    }

    pub(super) fn get_mut(&mut self, id: QueueId) -> ModelResult<&mut ExactQueueIo> {
        self.contexts
            .get_mut(id.raw())
            .and_then(|queue| queue.io.as_mut())
            .ok_or(ModelError::UnknownQueue(id))
    }
}
