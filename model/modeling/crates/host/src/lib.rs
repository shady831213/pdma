//! Async PDMA host model.

pub mod adapter;
pub mod config;
pub mod request;
pub mod runtime;
pub mod semantic;

pub use config::{
    EventRouting, QueueConfig, QueueConfigHandle, QueueDirectoryEntry, SoftwareOwnerId,
};
pub use request::{
    BroadcastCopyRequest, BroadcastDestinationRange, Command, CommandOperation, CopyRequest,
    StridedCopyRequest, VectorAddRequest, VectorBufferRange,
};
pub use semantic::{
    AsyncDriver, DriverError, DriverIrqRecord, DriverPort, DriverResult, ManualDriver, PortError,
    PortResult,
};
