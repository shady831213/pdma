//! Model-semantic PDMA vocabulary, independent of wire layout.
//!

mod capability;
pub mod completion;
pub mod dma;
mod error;
pub mod event;
pub mod ids;
pub mod irq;
pub mod queue;
pub mod resources;
pub mod trace;

pub use capability::{DeviceEpoch, DeviceRunState, DeviceStatus, EpochIrqSource, EpochQueue};
pub use error::{SemanticError, SemanticResult};

pub use completion::{CompletionRecord, CompletionStatus};
pub use dma::{DmaRange, DmaRangeStart};
pub use event::{EventErrorKind, EventRecord, FaultAddress, MemoryFault};
pub use ids::{BufferId, CommandId, CompletionRingId, DmaObjectId, QueueId};
pub use irq::{
    IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
    IrqVector,
};
pub use queue::{RingCursor, RingCursors};
pub use resources::ResourceConfig;
pub use trace::{TraceEvent, TraceEventKind, TraceOperation};
