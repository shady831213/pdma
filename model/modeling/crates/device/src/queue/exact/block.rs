use super::{ActiveQueue, ExactQueueIo};
use crate::abi::ring::PdmaControlError;
use crate::abi::ring::control::QueueStatus;
use crate::queue::{PdmaQueue, QueueBlock};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::QueueRingLayouts;
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, QueueActivation, QueueConfigHandle, QueueDirectoryEntry, QueueId,
};

pub(crate) struct PreparedExactQueue {
    context: PdmaQueue,
}

impl PreparedExactQueue {
    pub(crate) const fn new(activation: QueueActivation, layouts: QueueRingLayouts) -> Self {
        let queue = activation.queue().queue_id();
        let entry = QueueDirectoryEntry::new(
            queue,
            CompletionRingId::new(queue.raw()),
            activation.config(),
        );
        Self {
            context: PdmaQueue::exact(entry, ExactQueueIo::active(activation, layouts)),
        }
    }

    pub(crate) const fn queue_id(&self) -> QueueId {
        self.context.entry.queue_id()
    }

    pub(crate) fn status(&self) -> Result<QueueStatus, PdmaControlError> {
        self.context
            .io
            .exact()
            .ok_or(PdmaControlError::InvalidQueue)?
            .status()
    }

    fn into_queue(self) -> PdmaQueue {
        self.context
    }
}

impl QueueBlock {
    pub(crate) fn prepare_exact(
        &mut self,
        prepared: PreparedExactQueue,
    ) -> ModelResult<PreparedExactQueue> {
        self.contexts
            .try_reserve_exact(1)
            .map_err(|_| ModelError::DmaAllocationExhausted)?;
        let queue = prepared.queue_id();
        if queue != QueueId::new(self.contexts.len()) {
            return Err(ModelError::UnknownQueue(queue));
        }
        Ok(prepared)
    }

    pub(crate) fn install_exact(
        &mut self,
        epoch: DeviceEpoch,
        prepared: PreparedExactQueue,
    ) -> QueueConfigHandle {
        let context = prepared.into_queue();
        let entry = context.entry;
        self.contexts.push(context);
        QueueConfigHandle::new(epoch, entry)
    }

    pub(crate) fn validate_exact(&self, queue: QueueId) -> ModelResult<()> {
        let context = self.get(queue)?;
        if context.io.is_exact() {
            Ok(())
        } else {
            Err(ModelError::UnknownQueue(queue))
        }
    }

    pub(crate) fn exact_ring(&self, queue: QueueId) -> Result<&ActiveQueue, PdmaControlError> {
        let context = self
            .contexts
            .get(queue.raw())
            .filter(|context| context.entry.queue_id() == queue)
            .ok_or(PdmaControlError::InvalidQueue)?;
        context
            .io
            .exact()
            .ok_or(PdmaControlError::InvalidQueue)?
            .ring_state()
    }

    pub(crate) fn exact_ring_mut(
        &mut self,
        queue: QueueId,
    ) -> Result<&mut ActiveQueue, PdmaControlError> {
        let context = self
            .contexts
            .get_mut(queue.raw())
            .filter(|context| context.entry.queue_id() == queue)
            .ok_or(PdmaControlError::InvalidQueue)?;
        context
            .io
            .exact_mut()
            .ok_or(PdmaControlError::InvalidQueue)?
            .ring_state_mut()
    }
}
