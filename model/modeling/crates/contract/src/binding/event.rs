use crate::abi::AbiFault;
use crate::semantic::{CommandId, QueueId};

/// One exact fault tuple bound to semantic queue and command identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiEvent {
    queue_id: QueueId,
    command_id: CommandId,
    fault: AbiFault,
}

impl AbiEvent {
    /// Creates one event after runtime first-match classification.
    pub const fn new(queue_id: QueueId, command_id: CommandId, fault: AbiFault) -> Self {
        Self {
            queue_id,
            command_id,
            fault,
        }
    }

    /// Returns the submission-ring queue identity.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Returns the consumed descriptor command identity.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Returns the canonical exact fault tuple.
    pub const fn fault(self) -> AbiFault {
        self.fault
    }
}
