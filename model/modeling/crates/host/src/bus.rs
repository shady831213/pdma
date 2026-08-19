//! Canonical typed queue-status projection.

use pdma_model_contract::abi::RingCursor;
use pdma_model_contract::bridge::RingCursors;
use pdma_model_contract::semantic::EpochQueue;

/// Complete typed shadow state returned by canonical queue control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusQueueStatus {
    queue: EpochQueue,
    submission: RingCursors,
    captured_submission_producer: RingCursor,
    completion: RingCursors,
    event: RingCursors,
}

impl BusQueueStatus {
    /// Creates a status from typed queue shadows.
    pub const fn new(
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
    /// Submission producer and consumer.
    pub const fn submission(self) -> RingCursors {
        self.submission
    }
    /// Submission producer captured by the last doorbell.
    pub const fn captured_submission_producer(self) -> RingCursor {
        self.captured_submission_producer
    }
    /// Completion producer and consumer.
    pub const fn completion(self) -> RingCursors {
        self.completion
    }
    /// Event producer and consumer.
    pub const fn event(self) -> RingCursors {
        self.event
    }
}
