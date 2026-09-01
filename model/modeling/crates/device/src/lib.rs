//! Async PDMA device model runner.

pub mod abi;
pub mod adapter;
pub mod compat;
pub mod dma;
#[doc(hidden)]
pub mod execution;
#[doc(hidden)]
pub mod irq;
#[doc(hidden)]
pub mod queue;
#[doc(hidden)]
pub mod register_file;
#[doc(hidden)]
pub mod retirement;
pub mod runtime;
#[doc(hidden)]
pub mod scheduler;
mod status;
/// Explicit snapshot and construction support for device integration tests.
pub mod test_support;
#[doc(hidden)]
pub mod trace;

#[doc(hidden)]
pub mod device;

pub use abi::{
    DmaBatchContext, DmaWrite, MmioRegisterError, PdmaBatchError, PdmaControlError,
    PendingDmaResult, QueueBootstrapBatch, QueueResponseBytes, QueueStatus, SubmissionAcceptance,
    SubmissionBatch,
};
pub use dma::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
pub use runtime::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    DeviceCore, DeviceError, MmioRegisterEndpoint, ModelError, ModelResult, PdmaResetError,
    PdmaRunError, SyncDevice,
};
pub use status::{AsyncStallReason, AsyncStepStatus};
