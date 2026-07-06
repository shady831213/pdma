//! Vector-add scheduled work.

use pdma_model_contract::desc::VectorAddRequest;
use pdma_model_contract::ids::{CommandId, CompletionRingId, QueueId};

/// Internal scheduler output for deterministic vector-add execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledVectorAdd {
    queue_id: QueueId,
    command_id: CommandId,
    completion_ring_id: CompletionRingId,
    request: VectorAddRequest,
}

impl ScheduledVectorAdd {
    /// Creates an internal scheduled vector-add descriptor.
    pub const fn new(
        queue_id: QueueId,
        command_id: CommandId,
        completion_ring_id: CompletionRingId,
        request: VectorAddRequest,
    ) -> Self {
        Self {
            queue_id,
            command_id,
            completion_ring_id,
            request,
        }
    }

    /// Queue that carried the external command.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Command identifier assigned by the driver.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Completion ring where the result must be published.
    pub const fn completion_ring_id(self) -> CompletionRingId {
        self.completion_ring_id
    }

    /// Vector-add request payload.
    pub const fn request(self) -> VectorAddRequest {
        self.request
    }
}
