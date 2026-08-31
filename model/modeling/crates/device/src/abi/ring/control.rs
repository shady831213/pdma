//! Internal exact-ABI queue state and cursor transitions.

use super::PdmaControlError;
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, QueueRingLayouts};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochQueue, QueueActivation, RingCursor, RingCursors,
};

pub mod activation;
pub mod ring;
pub mod transactions;

pub(crate) use ring::{RingState, SubmissionRingState};
use ring::{exact_cursor, semantic_cursors};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveQueue {
    pub(crate) activation: QueueActivation,
    pub(crate) submission: SubmissionRingState,
    pub(crate) completion: RingState<AbiCompletion>,
    pub(crate) event: RingState<AbiEvent>,
}

/// Complete typed shadow state for one active DMA queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueStatus {
    queue: EpochQueue,
    submission: RingCursors,
    captured_submission_producer: RingCursor,
    completion: RingCursors,
    event: RingCursors,
}

impl QueueStatus {
    /// Epoch-qualified queue identity.
    pub const fn queue(self) -> EpochQueue {
        self.queue
    }

    /// Submission producer/consumer shadows.
    pub const fn submission(self) -> RingCursors {
        self.submission
    }

    /// Submission producer captured by the last doorbell.
    pub const fn captured_submission_producer(self) -> RingCursor {
        self.captured_submission_producer
    }

    /// Completion producer/consumer shadows.
    pub const fn completion(self) -> RingCursors {
        self.completion
    }

    /// Event producer/consumer shadows.
    pub const fn event(self) -> RingCursors {
        self.event
    }
}

#[derive(Debug)]
pub(crate) struct QueueControlState {
    epoch: DeviceEpoch,
    queues: Vec<ActiveQueue>,
}

impl QueueControlState {
    pub(crate) const fn new() -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            queues: Vec::new(),
        }
    }

    pub(crate) const fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }

    pub(crate) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.queues.clear();
    }

    pub(crate) fn queue(&self, queue: EpochQueue) -> Result<&ActiveQueue, PdmaControlError> {
        self.validate_queue(queue)?;
        self.queues
            .get(queue.queue_id().raw())
            .ok_or(PdmaControlError::InvalidQueue)
    }

    pub(crate) fn queue_mut(
        &mut self,
        queue: EpochQueue,
    ) -> Result<&mut ActiveQueue, PdmaControlError> {
        self.validate_queue(queue)?;
        self.queues
            .get_mut(queue.queue_id().raw())
            .ok_or(PdmaControlError::InvalidQueue)
    }

    fn validate_queue(&self, queue: EpochQueue) -> Result<(), PdmaControlError> {
        if queue.epoch() != self.epoch {
            Err(PdmaControlError::StaleEpoch {
                expected: self.epoch,
                actual: queue.epoch(),
            })
        } else if queue.queue_id().raw() >= self.queues.len() {
            Err(PdmaControlError::InvalidQueue)
        } else {
            Ok(())
        }
    }
}

impl ActiveQueue {
    const fn new(activation: QueueActivation, layouts: QueueRingLayouts) -> Self {
        Self {
            activation,
            submission: SubmissionRingState::new(layouts.submission()),
            completion: RingState::empty(layouts.completion()),
            event: RingState::empty(layouts.event()),
        }
    }

    pub(super) fn status(self) -> Result<QueueStatus, PdmaControlError> {
        Ok(QueueStatus {
            queue: self.activation.queue(),
            submission: semantic_cursors(self.submission.ring())?,
            captured_submission_producer: pdma_model_contract::semantic::RingCursor::new(
                self.submission.captured_producer().raw(),
            ),
            completion: semantic_cursors(self.completion)?,
            event: semantic_cursors(self.event)?,
        })
    }
}

pub(super) use activation::{activate_mapped, activate_staged};
