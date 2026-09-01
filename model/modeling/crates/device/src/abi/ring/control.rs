//! Internal exact-ABI queue state and cursor transitions.

use super::PdmaControlError;
use pdma_model_contract::semantic::{EpochQueue, RingCursor, RingCursors};

pub mod activation;
pub mod transactions;

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
    pub(crate) const fn new(
        queue: EpochQueue,
        submission: RingCursors,
        captured_submission_producer: RingCursor,
        completion: RingCursors,
        event: RingCursors,
    ) -> Self {
        Self {
            queue,
            submission,
            captured_submission_producer,
            completion,
            event,
        }
    }

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

pub(crate) use activation::{activate_mapped, activate_staged};
