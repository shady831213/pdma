//! Async PDMA device model runner.
//!
//! Register storage instrumentation is private to owner unit tests.
//!
//! ```compile_fail
//! use pdma_model_device::__register_image_allocations_for_test;
//! ```
//!
//! ```compile_fail
//! async fn inspect(handle: pdma_model_device::AsyncDeviceHandle) {
//!     handle.__inspect_register_image_for_test(|_| ()).await;
//! }
//! ```
//!
//! ```compile_fail
//! async fn mutate(handle: pdma_model_device::AsyncDeviceHandle) {
//!     handle.__mutate_register_image_for_test(|_| ()).await;
//! }
//! ```
//!
//! ```
//! use pdma_model_device::{AsyncDevice, AsyncStallReason, DeviceCore, ModelError};
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
mod error;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod irq;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private address helpers are shared across device systems without exposing a memory API"
)]
mod memory;
#[expect(
    clippy::redundant_pub_crate,
    reason = "decoded operands are shared across private hardware units"
)]
mod operation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod queue;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private root caps downstream API while pub(crate) descendants are shared across sibling device systems"
)]
mod runtime;
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
#[expect(
    clippy::redundant_pub_crate,
    reason = "test-only helpers are shared across private hardware test modules"
)]
mod test_support;

pub use device::DeviceCore;
pub use device::drive::HardwareStepStatus;
pub use error::{ModelError, ModelError as DeviceError, ModelResult};
pub use queue::exact::{PdmaControlError, PendingDmaResult};
pub use runtime::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncRunSnapshot, AsyncRunSummary,
    MmioRegisterEndpoint, MmioRegisterError,
};
pub use status::{AsyncStallReason, AsyncStepStatus};
