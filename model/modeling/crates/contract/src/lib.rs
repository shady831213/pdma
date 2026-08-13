//! Shared DMA-visible PDMA model contract vocabulary.
//!
//! This crate intentionally contains identifiers, records, and configuration
//! contracts only. It does not provide memory backing storage or address
//! translation behavior.
//!
//! New consumers should import model vocabulary through [`semantic`], fixed wire and
//! MMIO vocabulary through [`abi`], and cross-layer conversions through [`bridge`].
//! Root-level re-exports remain transitional for existing device, host, and scenario
//! consumers and are scheduled for removal after those consumers migrate.

pub mod abi;
pub mod bridge;
pub mod completion;
pub mod desc;
pub mod dma;
pub mod event;
pub mod ids;
pub mod irq;
pub mod queue;
pub mod resources;
pub mod semantic;
pub mod trace;

pub use abi::*;
pub use bridge::{
    AbiDiscovery, AbiEvent, AbiLimits, ControlRequest, DiscoveryRequest, PdmaMemoryRegion,
    QueueActivation, QueueControl, QueueCursorUpdate, QueueRingRegions, RingCursors, RingLayout,
    decode_completion, decode_event, decode_memory_address, encode_completion, encode_event,
    encode_id_value, encode_memory_address, encode_memory_address_value, encode_semantic_event,
};
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
pub use semantic::{
    DeviceEpoch, DeviceRunState, DeviceStatus, EpochIrqRoute, EpochIrqSource, EpochQueue,
    IrqControl, SemanticError, SemanticResult, WholeDeviceReset,
};
pub use trace::{TraceEvent, TraceEventKind, TraceOperation};
