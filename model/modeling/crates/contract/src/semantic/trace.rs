//! Shared trace event schema types.

use crate::semantic::ids::{CommandId, QueueId};

/// Operation carried by a traced command or scheduled work item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceOperation {
    /// Linear memory copy.
    Copy,
    /// Strided memory copy.
    StridedCopy,
    /// Broadcast copy.
    BroadcastCopy,
    /// Byte-wise wrapping vector add.
    VectorAdd,
}

/// Category of a structured trace event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventKind {
    /// Software submitted a command descriptor.
    CommandSubmitted,
    /// Software rang a queue doorbell.
    DoorbellRunnable,
    /// The scheduler lowered an external command.
    Scheduled,
    /// Command execution completed without an engine fault.
    ExecutionSucceeded,
    /// Command execution faulted.
    ExecutionFaulted,
    /// A completion record was published.
    CompletionPublished,
    /// An event record was published.
    EventPublished,
    /// Software polled a completion record.
    CompletionPolled,
    /// Software polled an event path.
    EventPolled,
    /// A PDMA-local IRQ source latched a cause.
    IrqSourceLatched,
    /// The shared device IRQ line transitioned to asserted.
    IrqLineAsserted,
    /// The shared device IRQ line transitioned to deasserted.
    IrqLineDeasserted,
    /// The device emitted one discrete IRQ message.
    IrqMessageEmitted,
    /// Software acknowledged a PDMA-local IRQ cause.
    IrqDeviceAck,
    /// Host delivery began one handler turn.
    IrqHandlerStarted,
    /// One host handler future returned.
    IrqHandlerCompleted,
    /// A status step observed backpressure.
    Stall,
    /// A status step found no runnable work.
    Idle,
}

/// One deterministic in-memory trace event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    sequence: usize,
    kind: TraceEventKind,
    operation: Option<TraceOperation>,
    queue_id: Option<QueueId>,
    command_id: Option<CommandId>,
    detail: &'static str,
}

impl TraceOperation {
    /// Stable operation label used by JSON trace export.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::StridedCopy => "StridedCopy",
            Self::BroadcastCopy => "BroadcastCopy",
            Self::VectorAdd => "VectorAdd",
        }
    }
}

impl TraceEventKind {
    /// Stable event-kind label used by JSON trace export.
    pub const fn name(self) -> &'static str {
        match self {
            Self::CommandSubmitted => "command submitted",
            Self::DoorbellRunnable => "doorbell runnable",
            Self::Scheduled => "scheduled",
            Self::ExecutionSucceeded => "execution Success",
            Self::ExecutionFaulted => "execution Error",
            Self::CompletionPublished => "completion published",
            Self::EventPublished => "event published",
            Self::CompletionPolled => "completion poll",
            Self::EventPolled => "event poll",
            Self::IrqSourceLatched => "IRQ source latched",
            Self::IrqLineAsserted => "IRQ line asserted",
            Self::IrqLineDeasserted => "IRQ line deasserted",
            Self::IrqMessageEmitted => "IRQ message emitted",
            Self::IrqDeviceAck => "IRQ device ack",
            Self::IrqHandlerStarted => "IRQ handler started",
            Self::IrqHandlerCompleted => "IRQ handler completed",
            Self::Stall => "Stall",
            Self::Idle => "idle",
        }
    }
}

impl TraceEvent {
    /// Creates a trace event record.
    pub const fn new(
        sequence: usize,
        kind: TraceEventKind,
        operation: Option<TraceOperation>,
        queue_id: Option<QueueId>,
        command_id: Option<CommandId>,
        detail: &'static str,
    ) -> Self {
        Self {
            sequence,
            kind,
            operation,
            queue_id,
            command_id,
            detail,
        }
    }

    /// Deterministic event sequence ordinal used as JSON `ts`.
    pub const fn sequence(&self) -> usize {
        self.sequence
    }

    /// Event category.
    pub const fn kind(&self) -> TraceEventKind {
        self.kind
    }

    /// Operation, when the event is command-specific.
    pub const fn operation(&self) -> Option<TraceOperation> {
        self.operation
    }

    /// Queue associated with this event, when applicable.
    pub const fn queue_id(&self) -> Option<QueueId> {
        self.queue_id
    }

    /// Command associated with this event, when applicable.
    pub const fn command_id(&self) -> Option<CommandId> {
        self.command_id
    }

    /// Static detail string for the event.
    pub const fn detail(&self) -> &'static str {
        self.detail
    }

    /// Stable category label for the JSON trace `cat` field.
    pub const fn operation_name(&self) -> &'static str {
        match self.operation {
            Some(operation) => operation.name(),
            None => self.kind.name(),
        }
    }
}
