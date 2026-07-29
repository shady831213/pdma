//! Shared DMA-visible PDMA model contract vocabulary.
//!
//! This crate intentionally contains identifiers, records, and configuration
//! contracts only. It does not provide memory backing storage or address
//! translation behavior.

pub mod abi;
pub mod completion;
pub mod desc;
pub mod dma;
pub mod error;
pub mod event;
pub mod ids;
pub mod irq;
pub mod queue;
pub mod resources;
pub mod trace;

pub use abi::{DeviceEpoch, EpochIrqSource};
pub use completion::{CompletionRecord, CompletionStatus};
pub use desc::{
    BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange, CopyCommand,
    CopyRequest, ExternalCommand, StridedCopyCommand, StridedCopyRequest, VectorAddCommand,
    VectorAddRequest, VectorBufferRange,
};
pub use dma::{DmaRange, DmaRangeStart};
pub use error::{ModelError, ModelResult};
pub use event::{EventErrorKind, EventRecord, MemoryFault};
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
