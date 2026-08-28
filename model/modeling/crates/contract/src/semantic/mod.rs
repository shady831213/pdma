//! Model-semantic PDMA vocabulary, independent of wire layout.
//!

mod capability;
pub mod completion;
pub mod desc;
pub mod dma;
mod error;
pub mod event;
pub mod ids;
pub mod irq;
pub mod queue;
pub mod resources;
pub mod trace;

pub use capability::{
    DeviceEpoch, DeviceRunState, DeviceStatus, EpochIrqRoute, EpochIrqSource, EpochQueue,
    IrqControl, WholeDeviceReset,
};
pub use error::{SemanticError, SemanticResult};

pub use completion::{CompletionRecord, CompletionStatus};
pub use desc::{
    BroadcastCopyRequest, BroadcastDestinationRange, Command, CommandOperation, CopyRequest,
    StridedCopyRequest, VectorAddRequest, VectorBufferRange,
};
pub use dma::{DmaRange, DmaRangeStart};
pub use event::{EventErrorKind, EventRecord, FaultAddress, MemoryFault};
pub use ids::{BufferId, CommandId, CompletionRingId, DmaObjectId, QueueId};
pub use irq::{
    IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
    IrqVector,
};
pub use queue::{
    EventRouting, QueueActivation, QueueConfig, QueueConfigHandle, QueueControl, QueueCursorUpdate,
    QueueDirectoryEntry, QueueRingObjects, RingCursor, RingCursors, SoftwareOwnerId,
};
pub use resources::ResourceConfig;
pub use trace::{TraceEvent, TraceEventKind, TraceOperation};
