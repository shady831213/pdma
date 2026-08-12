use crate::semantic::{DeviceEpoch, EpochQueue, QueueConfig, QueueId};

use crate::abi::{AbiControlError, RingCursor, RingRecord};
use crate::bridge::RingLayout;

/// The three explicit DMA ring layouts committed during queue activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueRingRegions {
    submission: RingLayout,
    completion: RingLayout,
    event: RingLayout,
}

impl QueueRingRegions {
    /// Groups staged submission, completion, and event rings.
    pub const fn new(submission: RingLayout, completion: RingLayout, event: RingLayout) -> Self {
        Self {
            submission,
            completion,
            event,
        }
    }

    /// Submission-ring layout.
    pub const fn submission(self) -> RingLayout {
        self.submission
    }

    /// Completion-ring layout.
    pub const fn completion(self) -> RingLayout {
        self.completion
    }

    /// Queue-local event-ring layout.
    pub const fn event(self) -> RingLayout {
        self.event
    }
}

/// One fully validated atomic queue-activation value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueActivation {
    queue: EpochQueue,
    config: QueueConfig,
    rings: QueueRingRegions,
}

impl QueueActivation {
    /// Validates canonical record roles and all three configured depths.
    pub fn try_new(
        epoch: DeviceEpoch,
        queue_id: QueueId,
        config: QueueConfig,
        rings: QueueRingRegions,
    ) -> Result<Self, AbiControlError> {
        Self::validate_record(rings.submission(), RingRecord::Submission)?;
        Self::validate_record(rings.completion(), RingRecord::Completion)?;
        Self::validate_record(rings.event(), RingRecord::Event)?;
        Self::validate_depth(
            config.command_depth(),
            rings.submission().depth(),
            "submission",
        )?;
        Self::validate_depth(
            config.completion_depth(),
            rings.completion().depth(),
            "completion",
        )?;
        let expected_event = config.event_depth();
        let actual_event = rings.event().depth();
        if usize::try_from(actual_event) != Ok(expected_event) {
            return Err(AbiControlError::EventDepthMismatch {
                expected: expected_event,
                actual: actual_event,
            });
        }
        Ok(Self {
            queue: EpochQueue::new(epoch, queue_id),
            config,
            rings,
        })
    }

    fn validate_record(layout: RingLayout, expected: RingRecord) -> Result<(), AbiControlError> {
        if layout.record() == expected {
            Ok(())
        } else {
            Err(AbiControlError::UnexpectedRingRecord {
                expected: expected.record_size(),
                actual: layout.record().record_size(),
            })
        }
    }

    fn validate_depth(
        expected: usize,
        actual: u32,
        ring: &'static str,
    ) -> Result<(), AbiControlError> {
        if usize::try_from(actual) == Ok(expected) {
            Ok(())
        } else {
            Err(AbiControlError::QueueDepthMismatch {
                ring,
                expected,
                actual,
            })
        }
    }

    /// Epoch-qualified queue identity.
    pub const fn queue(self) -> EpochQueue {
        self.queue
    }

    /// Semantic queue configuration.
    pub const fn config(self) -> QueueConfig {
        self.config
    }

    /// Exact staged ring regions.
    pub const fn rings(self) -> QueueRingRegions {
        self.rings
    }
}

/// One epoch-qualified queue cursor publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueCursorUpdate {
    queue: EpochQueue,
    cursor: RingCursor,
}

impl QueueCursorUpdate {
    /// Creates a cursor publication.
    pub const fn new(queue: EpochQueue, cursor: RingCursor) -> Self {
        Self { queue, cursor }
    }

    /// Target queue generation and identity.
    pub const fn queue(self) -> EpochQueue {
        self.queue
    }

    /// Proposed monotonic cursor shadow.
    pub const fn cursor(self) -> RingCursor {
        self.cursor
    }
}

/// Typed queue status and cursor operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueControl {
    /// Read every cursor shadow for one queue.
    ReadStatus(EpochQueue),
    /// Publish host submission acceptance.
    PublishSubmissionProducer(QueueCursorUpdate),
    /// Capture the host submission producer at a doorbell.
    CaptureSubmissionProducer(QueueCursorUpdate),
    /// Publish device submission consumption.
    PublishSubmissionConsumer(QueueCursorUpdate),
    /// Publish device completion production.
    PublishCompletionProducer(QueueCursorUpdate),
    /// Publish host completion consumption.
    PublishCompletionConsumer(QueueCursorUpdate),
    /// Publish device event production.
    PublishEventProducer(QueueCursorUpdate),
    /// Publish host event consumption.
    PublishEventConsumer(QueueCursorUpdate),
}

impl QueueControl {
    /// Creates a host submission-producer publication.
    pub const fn publish_submission_producer(
        epoch: DeviceEpoch,
        queue_id: QueueId,
        cursor: RingCursor,
    ) -> Self {
        Self::PublishSubmissionProducer(QueueCursorUpdate::new(
            EpochQueue::new(epoch, queue_id),
            cursor,
        ))
    }
}
