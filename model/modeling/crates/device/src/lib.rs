//! Async PDMA device model runner.

pub mod abi;
pub mod adapter;
pub mod compat;
/// Internal execution model shared by semantic and exact-ABI frontends.
pub mod engine;
#[doc(hidden)]
pub mod hardware;
pub mod runtime;
/// Explicit snapshot and construction support for device integration tests.
pub mod test_support;

pub use abi::dma::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
pub use abi::{
    DmaBatchContext, DmaWrite, MmioRegisterEndpoint, MmioRegisterError, PdmaBatchError,
    PdmaControlError, PendingDmaResult, QueueBootstrapBatch, QueueResponseBytes, QueueStatus,
    SubmissionAcceptance, SubmissionBatch,
};
pub use runtime::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    AsyncStallReason, AsyncStepStatus, DeviceCore, DeviceError, ModelError, ModelResult,
    PdmaResetError, PdmaRunError, SyncDevice,
};
