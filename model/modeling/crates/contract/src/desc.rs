//! Shared external command descriptor schema.

#[path = "desc/broadcast.rs"]
mod broadcast;
#[path = "desc/copy.rs"]
mod copy;
#[path = "desc/vector_add.rs"]
mod vector_add;

pub use broadcast::{BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange};
pub use copy::{CopyCommand, CopyRequest, StridedCopyCommand, StridedCopyRequest};
pub use vector_add::{VectorAddCommand, VectorAddRequest, VectorBufferRange};

/// Hardware-visible command descriptor emitted by the driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalCommand {
    /// Linear memory-to-memory copy.
    Copy(CopyCommand),
    /// 2D strided memory-to-memory copy.
    StridedCopy(StridedCopyCommand),
    /// One source range copied to multiple destination ranges.
    BroadcastCopy(BroadcastCopyCommand),
    /// Byte-wise wrapping vector add over two source ranges.
    VectorAdd(VectorAddCommand),
}

#[cfg(test)]
mod tests {
    use super::{CopyCommand, CopyRequest, ExternalCommand};
    use crate::semantic::ids::{BufferId, CommandId, CompletionRingId, QueueId};

    #[test]
    fn external_copy_schema_is_owned_by_contract() {
        // Given: a shared copy request built from contract IDs.
        let request = CopyRequest::new(BufferId::new(1), BufferId::new(2), 64);
        let command = CopyCommand::new(
            CommandId::new(3),
            QueueId::new(4),
            CompletionRingId::new(5),
            request,
        );

        // When: the request is wrapped in the external command schema.
        let external = ExternalCommand::Copy(command);

        // Then: contract exposes the external schema without device types.
        assert_eq!(external, ExternalCommand::Copy(command));
    }
}
