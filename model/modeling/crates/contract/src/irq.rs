//! Shared IRQ contract vocabulary.

use crate::ids::{CompletionRingId, QueueId};

/// Host-system interrupt vector identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IrqVector(usize);

/// Host-observable interrupt line identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IrqLine {
    vector: IrqVector,
}

/// PDMA-local source that can latch an interrupt cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IrqSource {
    /// Completion publication source for a completion ring.
    Completion(CompletionRingId),
    /// Queue-local event publication source.
    Event(QueueId),
}

/// Latched interrupt cause visible at the host/device boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrqCause {
    line: IrqLine,
    source: IrqSource,
}

/// Snapshot of one pending interrupt cause and line delivery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrqSnapshot {
    cause: IrqCause,
    asserted: bool,
}

impl IrqVector {
    /// Creates a host-system interrupt vector from its raw value.
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the raw integer value.
    pub const fn raw(self) -> usize {
        self.0
    }
}

impl IrqLine {
    /// Creates a host-observable interrupt line from a vector.
    pub const fn new(vector: IrqVector) -> Self {
        Self { vector }
    }

    /// System vector routed by this line.
    pub const fn vector(self) -> IrqVector {
        self.vector
    }
}

impl IrqCause {
    /// Creates a latched cause for `source` on `line`.
    pub const fn new(line: IrqLine, source: IrqSource) -> Self {
        Self { line, source }
    }

    /// Host-observable line carrying this cause.
    pub const fn line(self) -> IrqLine {
        self.line
    }

    /// PDMA-local source that caused the interrupt.
    pub const fn source(self) -> IrqSource {
        self.source
    }
}

impl IrqSnapshot {
    /// Creates an interrupt observation snapshot.
    pub const fn new(cause: IrqCause, asserted: bool) -> Self {
        Self { cause, asserted }
    }

    /// Pending cause captured by this snapshot.
    pub const fn cause(self) -> IrqCause {
        self.cause
    }

    /// Whether the modeled IRQ line is asserted for delivery.
    pub const fn asserted(self) -> bool {
        self.asserted
    }
}
