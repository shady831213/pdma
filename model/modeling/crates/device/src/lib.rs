//! Hardware-behavior PDMA device model.
//!
//! This crate owns deterministic device execution behavior, including memory
//! backing, queues, rings, scheduling, engines, arbitration, resources, and
//! runtime tracing.

pub mod arbitration;
pub mod device;
pub mod engines;
pub mod memory;
pub mod resources;
pub mod rings;
pub mod runner;
pub mod scheduled;
pub mod scheduler;
pub mod trace;
mod vector_add;

pub use arbitration::{ArbitrationPolicy, QueueArbiter};
pub use device::{PdmaDevice, RunStatusSummary, RunSummary, StallReason, StepStatus};
pub use memory::Memory;
pub use rings::Ring;
pub use runner::{
    SyncDeviceHandle, SyncDeviceRunner, SyncRunJoinError, SyncRunSnapshot, SyncRunSummary,
};
pub use scheduled::{CopyChunk, ScheduledCopy, ScheduledWork};
pub use trace::TraceSink;
