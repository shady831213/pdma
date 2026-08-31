//! Semantic PDMA command requests independent of wire layout.

mod broadcast;
mod copy;
mod vector_add;

pub use broadcast::{BroadcastCopyRequest, BroadcastDestinationRange};
pub use copy::{CopyRequest, StridedCopyRequest};
pub use vector_add::{VectorAddRequest, VectorBufferRange};

use crate::semantic::{CommandId, TraceOperation};

/// One semantic command with driver-assigned identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    command_id: CommandId,
    operation: CommandOperation,
}

impl Command {
    /// Creates a semantic command.
    pub const fn new(command_id: CommandId, operation: CommandOperation) -> Self {
        Self {
            command_id,
            operation,
        }
    }

    /// Driver-assigned command identity.
    pub const fn command_id(&self) -> CommandId {
        self.command_id
    }

    /// Requested operation.
    pub const fn operation(&self) -> &CommandOperation {
        &self.operation
    }

    /// Consumes the command and returns its requested operation.
    pub fn into_operation(self) -> CommandOperation {
        self.operation
    }
}

/// Closed semantic operation vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOperation {
    /// Linear memory-to-memory copy.
    Copy(CopyRequest),
    /// 2D strided memory-to-memory copy.
    StridedCopy(StridedCopyRequest),
    /// One source range copied to multiple destination ranges.
    BroadcastCopy(BroadcastCopyRequest),
    /// Byte-wise wrapping vector add over two source ranges.
    VectorAdd(VectorAddRequest),
}

impl CommandOperation {
    /// Returns the stable trace category for this semantic operation.
    pub const fn trace_operation(&self) -> TraceOperation {
        match self {
            Self::Copy(_) => TraceOperation::Copy,
            Self::StridedCopy(_) => TraceOperation::StridedCopy,
            Self::BroadcastCopy(_) => TraceOperation::BroadcastCopy,
            Self::VectorAdd(_) => TraceOperation::VectorAdd,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, CommandOperation, CopyRequest};
    use crate::semantic::ids::{BufferId, CommandId};

    #[test]
    fn semantic_command_keeps_identity_and_operation_without_queue_routing() {
        // Given: a semantic copy request and driver-assigned command identity.
        let request = CopyRequest::new(BufferId::new(1), BufferId::new(2), 64);

        // When: the request is wrapped in the semantic command union.
        let command = Command::new(CommandId::new(3), CommandOperation::Copy(request));

        // Then: only command identity and operation semantics are retained.
        assert_eq!(command.command_id(), CommandId::new(3));
        assert_eq!(command.operation(), &CommandOperation::Copy(request));
    }
}
