use super::{ActiveQueue, ExactQueueIo, PdmaControlError};
use crate::queue::QueueBlock;
use pdma_model_contract::abi::QueueRingLayouts;
use pdma_model_contract::semantic::QueueId;

impl QueueBlock {
    pub(crate) fn install_exact(
        &mut self,
        queue: QueueId,
        layouts: QueueRingLayouts,
    ) -> Result<(), PdmaControlError> {
        let context = self
            .contexts
            .get_mut(queue.raw())
            .ok_or(PdmaControlError::InvalidQueue)?;
        if context.io.is_some() {
            return Err(PdmaControlError::InvalidTransaction);
        }
        context.io = Some(ExactQueueIo::active(layouts));
        context.registers.enable = 1.into();
        Ok(())
    }

    pub(crate) fn exact_ring(&self, queue: QueueId) -> Result<&ActiveQueue, PdmaControlError> {
        self.get(queue)
            .map(ExactQueueIo::ring_state)
            .map_err(|_| PdmaControlError::InvalidQueue)
    }

    pub(crate) fn exact_ring_mut(
        &mut self,
        queue: QueueId,
    ) -> Result<&mut ActiveQueue, PdmaControlError> {
        self.get_mut(queue)
            .map(ExactQueueIo::ring_state_mut)
            .map_err(|_| PdmaControlError::InvalidQueue)
    }
}
