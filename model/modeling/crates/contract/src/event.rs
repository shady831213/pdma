//! Event records returned from the PDMA device to the driver.

use crate::semantic::ids::{BufferId, CommandId, QueueId};

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
    buffer: BufferId,
    error_kind: EventErrorKind,
    fault_address: Option<FaultAddress>,
}

impl MemoryFault {
    /// Creates a memory fault from the original model error and faulting buffer.
    pub const fn new(buffer: BufferId, error_kind: EventErrorKind) -> Self {
        Self {
            buffer,
            error_kind,
            fault_address: None,
        }
    }

    /// Creates a memory fault with an address when a fixed-width fault locator is known.
    pub const fn with_address(
        buffer: BufferId,
        error_kind: EventErrorKind,
        fault_address: FaultAddress,
    ) -> Self {
        Self {
            buffer,
            error_kind,
            fault_address: Some(fault_address),
        }
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
    pub const fn fault_address(self) -> Option<FaultAddress> {
        self.fault_address
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
        Self::from_memory_fault(queue_id, command_id, MemoryFault::new(buffer, error_kind))
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
    pub const fn fault_address(self) -> Option<FaultAddress> {
        self.fault.fault_address()
    }
}
/// Runtime-neutral semantic address associated with a memory fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultAddress(u64);

impl FaultAddress {
    /// Creates a semantic address from its complete numeric value.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the complete numeric value.
    pub const fn raw(self) -> u64 {
        self.0
    }
}
