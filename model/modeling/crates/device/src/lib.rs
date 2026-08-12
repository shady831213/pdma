//! Async PDMA device model runner.

mod dma_space;
mod error;
mod runner;
pub mod trace;

pub use dma_space::{DmaSpace, DmaSpaceError, DmaStage};
pub use error::{ModelError, ModelResult};
pub use runner::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    AsyncStallReason, AsyncStepStatus, BusControlError, DmaBatchContext, DmaWrite, MmioBatchError,
    PendingDmaResult, QueueBootstrapBatch, QueueResponseBytes, QueueStatus, SubmissionAcceptance,
    SubmissionBatch, SyncDevice,
};
pub use trace::TraceSink;
