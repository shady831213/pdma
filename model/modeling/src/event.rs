//! Event records returned from the PDMA device to the driver.

use crate::ModelError;
use crate::ids::{BufferId, CommandId, QueueId};

/// Software-visible recoverable command error kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventErrorKind {
    /// A command referenced a buffer that does not exist.
    UnknownBuffer,
    /// A checked range calculation overflowed.
    RangeOverflow,
    /// A command touched bytes outside a buffer.
    OutOfBounds {
        /// Buffer length in bytes.
        len: usize,
        /// Requested exclusive range end.
        end: usize,
    },
}

/// Memory fault metadata carried from an engine to the event unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryFault {
    source: ModelError,
    buffer: BufferId,
    error_kind: EventErrorKind,
}

impl MemoryFault {
    /// Creates a memory fault from the original model error and faulting buffer.
    pub(crate) const fn new(
        source: ModelError,
        buffer: BufferId,
        error_kind: EventErrorKind,
    ) -> Self {
        Self {
            source,
            buffer,
            error_kind,
        }
    }

    /// Original model error that should be returned to the caller.
    pub const fn source(self) -> ModelError {
        self.source
    }

    /// Buffer associated with the memory fault.
    pub const fn buffer(self) -> BufferId {
        self.buffer
    }

    /// Software-visible event error kind.
    pub const fn error_kind(self) -> EventErrorKind {
        self.error_kind
    }
}

/// Software-visible event record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRecord {
    queue_id: QueueId,
    command_id: CommandId,
    error_kind: EventErrorKind,
    buffer: BufferId,
}

impl EventRecord {
    /// Creates an event record.
    pub const fn new(
        queue_id: QueueId,
        command_id: CommandId,
        error_kind: EventErrorKind,
        buffer: BufferId,
    ) -> Self {
        Self {
            queue_id,
            command_id,
            error_kind,
            buffer,
        }
    }

    /// Queue that produced the event.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Command that produced the event.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Recoverable command error kind.
    pub const fn error_kind(self) -> EventErrorKind {
        self.error_kind
    }

    /// Buffer associated with the error.
    pub const fn buffer(self) -> BufferId {
        self.buffer
    }
}
