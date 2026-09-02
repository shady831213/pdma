//! Async PDMA device model runner.

#[allow(clippy::redundant_pub_crate)]
mod adapter;
pub mod dma;
#[allow(clippy::redundant_pub_crate)]
mod execution;
#[allow(clippy::redundant_pub_crate)]
mod interface;
#[allow(clippy::redundant_pub_crate)]
mod irq;
#[allow(clippy::redundant_pub_crate)]
mod queue;
#[allow(clippy::redundant_pub_crate)]
mod register_file;
#[allow(clippy::redundant_pub_crate)]
mod retirement;
#[allow(clippy::redundant_pub_crate)]
mod runtime;
#[allow(clippy::redundant_pub_crate)]
mod scheduler;
mod status;
#[allow(clippy::redundant_pub_crate)]
mod trace;

#[allow(clippy::redundant_pub_crate)]
mod device;

pub use dma::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
pub use interface::batch::{
    DmaBatchContext, PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch,
};
pub use interface::mmio::MmioRegisterError;
pub use queue::exact::{
    DmaWrite, PdmaControlError, PendingDmaResult, QueueStatus, SubmissionAcceptance,
};
pub use runtime::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    DeviceCore, DeviceError, MmioRegisterEndpoint, ModelError, ModelResult,
};
pub use status::{AsyncStallReason, AsyncStepStatus};
