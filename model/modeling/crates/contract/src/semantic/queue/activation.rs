use crate::semantic::{BufferId, DeviceEpoch, EpochQueue, QueueConfig, QueueId};

/// DMA objects assigned to the three queue ring roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueRingObjects {
    submission: BufferId,
    completion: BufferId,
    event: BufferId,
}

impl QueueRingObjects {
    /// Assigns one semantic DMA object to each queue ring role.
    pub const fn new(submission: BufferId, completion: BufferId, event: BufferId) -> Self {
        Self {
            submission,
            completion,
            event,
        }
    }

    /// Submission-ring DMA object.
    pub const fn submission(self) -> BufferId {
        self.submission
    }

    /// Completion-ring DMA object.
    pub const fn completion(self) -> BufferId {
        self.completion
    }

    /// Event-ring DMA object.
    pub const fn event(self) -> BufferId {
        self.event
    }
}

/// Semantic request to activate one queue over three mapped DMA objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueActivation {
    queue: EpochQueue,
    config: QueueConfig,
    rings: QueueRingObjects,
}

impl QueueActivation {
    /// Creates a queue activation request without embedding ABI layout state.
    pub const fn new(
        epoch: DeviceEpoch,
        queue_id: QueueId,
        config: QueueConfig,
        rings: QueueRingObjects,
    ) -> Self {
        Self {
            queue: EpochQueue::new(epoch, queue_id),
            config,
            rings,
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

    /// DMA objects assigned to queue ring roles.
    pub const fn rings(self) -> QueueRingObjects {
        self.rings
    }
}
