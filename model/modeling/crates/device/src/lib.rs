//! Async PDMA device model runner.
//!
//! ```
//! use pdma_model_device::{AsyncDevice, AsyncStallReason, DeviceCore, MappedDmaRegion, ModelError};
//! ```
//!
//! ```compile_fail
//! use pdma_model_device::PipelineFixture;
//! ```
//!
//! ```compile_fail
//! use pdma_model_device::{PipelineRunOutcome, PipelineTerminal};
//! ```
//!
//! ```compile_fail
//! use pdma_model_device::PdmaCommandPipeline;
//! ```
//!
//! ```compile_fail
//! use pdma_model_device::device::command_pipeline::PdmaCommandPipeline;
//! use pdma_model_device::device::command_pipeline::execute::Copy;
//! use pdma_model_device::device::command_pipeline::fetch::ProbeFetch;
//! use pdma_model_device::device::command_pipeline::retire::Success;
//! ```

#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod adapter;
pub mod dma;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod execution;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod interface;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod irq;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod queue;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod registers;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod retirement;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod runtime;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod scheduler;
mod status;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod trace;

#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod device;

#[cfg(test)]
#[path = "registers/baseline_manifest.rs"]
mod register_file;

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

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn __register_image_allocations_for_test() -> (usize, usize) {
    registers::allocation_observation()
}
