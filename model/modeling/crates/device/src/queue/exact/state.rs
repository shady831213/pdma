use super::ring_state::{RingState, SubmissionRingState, semantic_cursors};
use crate::abi::ring::{PdmaControlError, QueueStatus};
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, QueueRingLayouts};
use pdma_model_contract::semantic::QueueActivation;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveQueue {
    pub(crate) activation: QueueActivation,
    pub(crate) submission: SubmissionRingState,
    pub(crate) completion: RingState<AbiCompletion>,
    pub(crate) event: RingState<AbiEvent>,
}

impl ActiveQueue {
    pub(crate) const fn new(activation: QueueActivation, layouts: QueueRingLayouts) -> Self {
        Self {
            activation,
            submission: SubmissionRingState::new(layouts.submission()),
            completion: RingState::empty(layouts.completion()),
            event: RingState::empty(layouts.event()),
        }
    }

    pub(crate) fn status(self) -> Result<QueueStatus, PdmaControlError> {
        Ok(QueueStatus::new(
            self.activation.queue(),
            semantic_cursors(self.submission.ring())?,
            pdma_model_contract::semantic::RingCursor::new(
                self.submission.captured_producer().raw(),
            ),
            semantic_cursors(self.completion)?,
            semantic_cursors(self.event)?,
        ))
    }
}
