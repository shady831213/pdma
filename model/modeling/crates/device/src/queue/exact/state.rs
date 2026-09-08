use super::PdmaControlError;
use super::ring_state::{RingState, SubmissionRingState};
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, QueueRingLayouts};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveQueue {
    pub(crate) submission: SubmissionRingState,
    pub(crate) completion: RingState<AbiCompletion>,
    pub(crate) event: RingState<AbiEvent>,
}

impl ActiveQueue {
    pub(crate) const fn new(layouts: QueueRingLayouts) -> Self {
        Self {
            submission: SubmissionRingState::new(layouts.submission()),
            completion: RingState::empty(layouts.completion()),
            event: RingState::empty(layouts.event()),
        }
    }

    pub(crate) fn publish_submission_producer(
        &mut self,
        cursor: pdma_model_contract::abi::RingCursor,
    ) -> Result<bool, PdmaControlError> {
        let updated = self.submission.update_producer(cursor)?;
        let changed = updated != self.submission;
        self.submission = updated;
        Ok(changed)
    }

    pub(crate) fn capture_submission_producer(
        &mut self,
        cursor: pdma_model_contract::abi::RingCursor,
    ) -> Result<bool, PdmaControlError> {
        let changed = cursor != self.submission.captured_producer();
        self.submission = self.submission.capture(cursor)?;
        Ok(changed)
    }

    pub(crate) fn publish_completion_consumer(
        &mut self,
        cursor: pdma_model_contract::abi::RingCursor,
    ) -> Result<bool, PdmaControlError> {
        let updated = self.completion.update_consumer(cursor)?;
        let changed = updated != self.completion;
        self.completion = updated;
        Ok(changed)
    }

    pub(crate) fn publish_event_consumer(
        &mut self,
        cursor: pdma_model_contract::abi::RingCursor,
    ) -> Result<bool, PdmaControlError> {
        let updated = self.event.update_consumer(cursor)?;
        let changed = updated != self.event;
        self.event = updated;
        Ok(changed)
    }

    pub(crate) fn advance_submission_consumer(&mut self) -> Result<(), PdmaControlError> {
        let next = self.submission.ring().consumer().checked_increment(1)?;
        self.submission = self.submission.consume_captured(next)?;
        Ok(())
    }
}
