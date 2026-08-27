//! Async PDMA device model runner.

pub mod compat;
pub mod dma_space;
mod error;
mod runner;
/// Explicit snapshot and construction support for device integration tests.
pub mod test_support;
pub mod trace;

pub use dma_space::{PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
pub use error::{ModelError, ModelError as DeviceError, ModelResult, PdmaResetError, PdmaRunError};
pub use runner::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    AsyncStallReason, AsyncStepStatus, DeviceCore, DmaBatchContext, DmaWrite, MmioRegisterEndpoint,
    MmioRegisterError, PdmaBatchError, PdmaControlError, PendingDmaResult, QueueBootstrapBatch,
    QueueResponseBytes, QueueStatus, SubmissionAcceptance, SubmissionBatch, SyncDevice,
};
pub use trace::TraceSink;
