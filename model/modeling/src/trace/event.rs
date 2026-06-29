use crate::completion::{CompletionRecord, CompletionStatus};
use crate::desc::{ExternalCommand, ScheduledWork};
use crate::event::{EventErrorKind, EventRecord};
use crate::ids::{CommandId, QueueId};

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
    pub(crate) const fn from_external(command: &ExternalCommand) -> Self {
        match command {
            ExternalCommand::Copy(_) => Self::Copy,
            ExternalCommand::StridedCopy(_) => Self::StridedCopy,
            ExternalCommand::BroadcastCopy(_) => Self::BroadcastCopy,
            ExternalCommand::VectorAdd(_) => Self::VectorAdd,
        }
    }

    pub(crate) const fn from_work(work: &ScheduledWork) -> Self {
        match work {
            ScheduledWork::Copy(_) => Self::Copy,
            ScheduledWork::VectorAdd(_) => Self::VectorAdd,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::StridedCopy => "StridedCopy",
            Self::BroadcastCopy => "BroadcastCopy",
            Self::VectorAdd => "VectorAdd",
        }
    }
}

impl TraceEventKind {
    pub(crate) const fn name(self) -> &'static str {
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
            Self::Stall => "Stall",
            Self::Idle => "idle",
        }
    }
}

impl TraceEvent {
    pub(crate) const fn new(
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

    pub(crate) const fn operation_name(&self) -> &'static str {
        match self.operation {
            Some(operation) => operation.name(),
            None => self.kind.name(),
        }
    }
}

pub(super) const fn command_id(command: &ExternalCommand) -> CommandId {
    match command {
        ExternalCommand::Copy(copy) => copy.command_id(),
        ExternalCommand::StridedCopy(strided) => strided.command_id(),
        ExternalCommand::BroadcastCopy(broadcast) => broadcast.command_id(),
        ExternalCommand::VectorAdd(vector_add) => vector_add.command_id(),
    }
}

pub(super) const fn completion_detail(record: CompletionRecord) -> &'static str {
    match record.status() {
        CompletionStatus::Success => "status Success",
    }
}

pub(super) const fn event_detail(record: EventRecord) -> &'static str {
    match record.error_kind() {
        EventErrorKind::UnknownBuffer => "Error UnknownBuffer",
        EventErrorKind::RangeOverflow => "Error RangeOverflow",
        EventErrorKind::OutOfBounds { len: _, end: _ } => "Error OutOfBounds",
    }
}
