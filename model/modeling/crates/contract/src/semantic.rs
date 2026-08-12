//! Model-semantic PDMA vocabulary, independent of wire layout.
//!
//! The legacy crate-root modules and exports remain available during the
//! additive namespace transition.

mod capability;
mod error;

pub use capability::{
    DeviceEpoch, DeviceRunState, DeviceStatus, EpochIrqRoute, EpochIrqSource, EpochQueue,
    IrqControl, WholeDeviceReset,
};
pub use error::{SemanticError, SemanticResult};

pub use crate::{completion, desc, dma, event, ids, irq, queue, resources, trace};

pub use crate::completion::{CompletionRecord, CompletionStatus};
pub use crate::desc::{
    BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange, CopyCommand,
    CopyRequest, ExternalCommand, StridedCopyCommand, StridedCopyRequest, VectorAddCommand,
    VectorAddRequest, VectorBufferRange,
};
pub use crate::dma::{DmaRange, DmaRangeStart};
pub use crate::event::{EventErrorKind, EventRecord, FaultAddress, MemoryFault};
pub use crate::ids::{BufferId, CommandId, CompletionRingId, DmaObjectId, QueueId};
pub use crate::irq::{
    IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
    IrqVector,
};
pub use crate::queue::{
    EventRouting, QueueConfig, QueueConfigHandle, QueueDirectoryEntry, SoftwareOwnerId,
};
pub use crate::resources::ResourceConfig;
pub use crate::trace::{TraceEvent, TraceEventKind, TraceOperation};
