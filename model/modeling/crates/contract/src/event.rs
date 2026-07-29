//! Event records returned from the PDMA device to the driver.

use crate::abi::DmaAddress;
use crate::error::ModelError;
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
    fault_iova: Option<DmaAddress>,
}

impl MemoryFault {
    /// Creates a memory fault from the original model error and faulting buffer.
    pub const fn new(source: ModelError, buffer: BufferId, error_kind: EventErrorKind) -> Self {
        Self {
            source,
            buffer,
            error_kind,
            fault_iova: None,
        }
    }

    /// Creates a memory fault with an address when a fixed-width fault locator is known.
    pub const fn with_iova(
        source: ModelError,
        buffer: BufferId,
        error_kind: EventErrorKind,
        fault_iova: DmaAddress,
    ) -> Self {
        Self {
            source,
            buffer,
            error_kind,
            fault_iova: Some(fault_iova),
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

    /// Fixed-width fault locator when the semantic object maps to a known address.
    pub const fn fault_iova(self) -> Option<DmaAddress> {
        self.fault_iova
    }
}

/// Software-visible event record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRecord {
    queue_id: QueueId,
    command_id: CommandId,
    fault: MemoryFault,
}

impl EventRecord {
    /// Creates an event record.
    pub const fn new(
        queue_id: QueueId,
        command_id: CommandId,
        error_kind: EventErrorKind,
        buffer: BufferId,
    ) -> Self {
        let source = match error_kind {
            EventErrorKind::UnknownBuffer => ModelError::UnknownBuffer(buffer),
            EventErrorKind::RangeOverflow => ModelError::RangeOverflow,
            EventErrorKind::OutOfBounds { len, end } => {
                ModelError::OutOfBounds { buffer, len, end }
            }
        };
        Self::from_memory_fault(
            queue_id,
            command_id,
            MemoryFault::new(source, buffer, error_kind),
        )
    }

    /// Creates an event from a typed semantic memory fault.
    pub const fn from_memory_fault(
        queue_id: QueueId,
        command_id: CommandId,
        fault: MemoryFault,
    ) -> Self {
        Self {
            queue_id,
            command_id,
            fault,
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
        self.fault.error_kind()
    }

    /// Buffer associated with the error.
    pub const fn buffer(self) -> BufferId {
        self.fault.buffer()
    }

    /// Fixed-width fault locator when one is known for the semantic object.
    pub const fn fault_iova(self) -> Option<DmaAddress> {
        self.fault.fault_iova()
    }
}
