//! Async PDMA device model runner.

pub mod abi;
#[doc(hidden)]
pub mod adapter;
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
#[doc(hidden)]
pub mod trace;

#[doc(hidden)]
pub mod device;

pub use abi::{
    DmaBatchContext, MmioRegisterError, PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes,
    SubmissionBatch,
};
pub use dma::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
pub use queue::exact::{
    DmaWrite, PdmaControlError, PendingDmaResult, QueueStatus, SubmissionAcceptance,
};
pub use runtime::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    DeviceCore, DeviceError, MmioRegisterEndpoint, ModelError, ModelResult,
};
pub use status::{AsyncStallReason, AsyncStepStatus};
