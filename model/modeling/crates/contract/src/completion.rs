//! Completion records returned from the PDMA device to the driver.

use crate::semantic::ids::{CommandId, QueueId};

/// Completion status for the first linear-copy slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStatus {
    /// The command completed successfully.
    Success,
}

/// Software-visible completion record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionRecord {
    queue_id: QueueId,
    command_id: CommandId,
    status: CompletionStatus,
}

impl CompletionRecord {
    /// Creates a completion record.
    pub const fn new(queue_id: QueueId, command_id: CommandId, status: CompletionStatus) -> Self {
        Self {
            queue_id,
            command_id,
            status,
        }
    }

    /// Queue that produced the completion.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Completed command identifier.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Completion status.
    pub const fn status(self) -> CompletionStatus {
        self.status
    }
}
