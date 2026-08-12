//! Shared IRQ contract vocabulary.

use crate::ids::{CompletionRingId, QueueId};
use crate::{DeviceEpoch, EpochIrqSource};

/// Host-system interrupt vector identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IrqVector(usize);

/// Host-observable interrupt line identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IrqLine {
    vector: IrqVector,
}

/// Device interrupt delivery route selected for one PDMA-local source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IrqRoute {
    /// Stateful interrupt-line delivery.
    Line(IrqLine),
    /// Discrete message-signaled delivery identified by a host vector.
    Message(IrqVector),
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
    source: EpochIrqSource,
}

/// Snapshot of one pending interrupt cause and line delivery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrqSnapshot {
    cause: IrqCause,
    asserted: bool,
}

/// Source-scoped monotonic identity assigned to one discrete IRQ message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IrqMessageId(usize);

/// One discrete message-signaled IRQ delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrqMessage {
    id: IrqMessageId,
    vector: IrqVector,
    source: EpochIrqSource,
}

/// Device IRQ observation delivered across the host boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrqNotification {
    /// Stateful interrupt-line observation.
    Line(IrqSnapshot),
    /// Discrete message-signaled observation.
    Message(IrqMessage),
}

impl IrqNotification {
    /// Host-system vector selected for this delivery.
    pub const fn vector(self) -> IrqVector {
        match self {
            Self::Line(snapshot) => snapshot.cause().line().vector(),
            Self::Message(message) => message.vector(),
        }
    }

    /// Epoch-qualified PDMA-local source carried by this observation.
    pub const fn source_capability(self) -> EpochIrqSource {
        match self {
            Self::Line(snapshot) => snapshot.cause().source_capability(),
            Self::Message(message) => message.source_capability(),
        }
    }

    /// PDMA-local source identity used for correlation.
    pub const fn source(self) -> IrqSource {
        self.source_capability().source()
    }

    /// Device generation carried by this observation.
    pub const fn epoch(self) -> DeviceEpoch {
        self.source_capability().epoch()
    }
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
    pub const fn new(line: IrqLine, source: EpochIrqSource) -> Self {
        Self { line, source }
    }

    /// Host-observable line carrying this cause.
    pub const fn line(self) -> IrqLine {
        self.line
    }

    /// PDMA-local source that caused the interrupt.
    pub const fn source(self) -> IrqSource {
        self.source.source()
    }

    /// Epoch-qualified source capability captured by this cause.
    pub const fn source_capability(self) -> EpochIrqSource {
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

impl IrqMessageId {
    /// Creates a source-scoped message identity from its monotonic raw value.
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the raw value that is monotonic within one IRQ source.
    pub const fn raw(self) -> usize {
        self.0
    }
}

impl IrqMessage {
    /// Creates one message-signaled delivery observation.
    pub const fn new(id: IrqMessageId, vector: IrqVector, source: EpochIrqSource) -> Self {
        Self { id, vector, source }
    }

    /// Message identity that is monotonic within this message's source.
    pub const fn id(self) -> IrqMessageId {
        self.id
    }

    /// Host vector carried by the message.
    pub const fn vector(self) -> IrqVector {
        self.vector
    }

    /// PDMA-local source that emitted the message.
    pub const fn source(self) -> IrqSource {
        self.source.source()
    }

    /// Epoch-qualified source capability captured by this message.
    pub const fn source_capability(self) -> EpochIrqSource {
        self.source
    }
}
