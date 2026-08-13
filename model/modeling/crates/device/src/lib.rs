//! Async PDMA device model runner.

pub mod dma_space;
mod error;
mod runner;
pub mod trace;

pub use dma_space::{PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
pub use error::{ModelError, ModelResult};
pub use runner::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    AsyncStallReason, AsyncStepStatus, BusControlError, DeviceCore, DmaBatchContext, DmaWrite,
    MmioBatchError, MmioRegisterEndpoint, MmioRegisterError, PendingDmaResult, QueueBootstrapBatch,
    QueueResponseBytes, QueueStatus, SubmissionAcceptance, SubmissionBatch, SyncDevice,
};
pub use trace::TraceSink;

/// Compatibility facade retained while synchronous callers migrate independently.
pub mod compat {
    pub use crate::runner::SyncDevice;
}
