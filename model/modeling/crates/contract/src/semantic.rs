//! Model-semantic PDMA vocabulary, independent of wire layout.
//!

mod capability;
#[path = "completion.rs"]
pub mod completion;
#[path = "desc.rs"]
pub mod desc;
#[path = "dma.rs"]
pub mod dma;
mod error;
#[path = "event.rs"]
pub mod event;
#[path = "ids.rs"]
pub mod ids;
#[path = "irq.rs"]
pub mod irq;
#[path = "queue.rs"]
pub mod queue;
#[path = "resources.rs"]
pub mod resources;
#[path = "trace.rs"]
pub mod trace;

pub use capability::{
    DeviceEpoch, DeviceRunState, DeviceStatus, EpochIrqRoute, EpochIrqSource, EpochQueue,
    IrqControl, WholeDeviceReset,
};
pub use error::{SemanticError, SemanticResult};

pub use completion::{CompletionRecord, CompletionStatus};
pub use desc::{
    BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange, CopyCommand,
    CopyRequest, ExternalCommand, StridedCopyCommand, StridedCopyRequest, VectorAddCommand,
    VectorAddRequest, VectorBufferRange,
};
pub use dma::{DmaRange, DmaRangeStart};
pub use event::{EventErrorKind, EventRecord, FaultAddress, MemoryFault};
pub use ids::{BufferId, CommandId, CompletionRingId, DmaObjectId, QueueId};
pub use irq::{
    IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
    IrqVector,
};
pub use queue::{
    EventRouting, QueueConfig, QueueConfigHandle, QueueDirectoryEntry, SoftwareOwnerId,
};
pub use resources::ResourceConfig;
pub use trace::{TraceEvent, TraceEventKind, TraceOperation};
