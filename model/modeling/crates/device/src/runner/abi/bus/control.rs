use super::PdmaControlError;
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, QueueRingLayouts};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochQueue, QueueActivation, RingCursor, RingCursors,
};

pub(in crate::runner) mod activation;
mod ring;
pub(in crate::runner) mod transactions;

pub(in crate::runner) use ring::{RingState, SubmissionRingState};

#[derive(Debug, Clone, Copy)]
pub(in crate::runner) struct ActiveQueue {
    pub(in crate::runner) activation: QueueActivation,
    pub(in crate::runner) submission: SubmissionRingState,
    pub(in crate::runner) completion: RingState<AbiCompletion>,
    pub(in crate::runner) event: RingState<AbiEvent>,
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
pub(in crate::runner) struct BusControlState {
    epoch: DeviceEpoch,
    queues: Vec<ActiveQueue>,
}

impl BusControlState {
    pub(crate) const fn new() -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            queues: Vec::new(),
        }
    }

    pub(in crate::runner) const fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }

    pub(in crate::runner) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.queues.clear();
    }

    pub(in crate::runner) fn queue(
        &self,
        queue: EpochQueue,
    ) -> Result<&ActiveQueue, PdmaControlError> {
        self.validate_queue(queue)?;
        self.queues
            .get(queue.queue_id().raw())
            .ok_or(PdmaControlError::InvalidQueue)
    }

    pub(in crate::runner) fn queue_mut(
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
    fn new(
        activation: QueueActivation,
        layouts: QueueRingLayouts,
    ) -> Result<Self, PdmaControlError> {
        Ok(Self {
            activation,
            submission: SubmissionRingState::new(layouts.submission())?,
            completion: RingState::new(layouts.completion())?,
            event: RingState::new(layouts.event())?,
        })
    }

    pub(super) const fn status(self) -> QueueStatus {
        QueueStatus {
            queue: self.activation.queue(),
            submission: self.submission.ring().cursors(),
            captured_submission_producer: self.submission.captured_producer(),
            completion: self.completion.cursors(),
            event: self.event.cursors(),
        }
    }
}

pub(super) use activation::{activate_mapped, activate_staged};
