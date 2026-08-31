//! Tokio-backed asynchronous hardware runner.

mod actor;
mod core;
mod error;
mod global_api;
pub mod irq;
mod manual;
mod memory;
mod queue;
mod reset;
pub mod state;
mod test_support;
mod types;
mod wait;
mod worker;

pub(crate) use crate::abi::MmioRegisterEndpoint;
use crate::abi::dma::PdmaMemoryError;
pub(crate) use crate::abi::ring::{PdmaControlError, PendingDmaResult, QueueStatus};
pub use actor::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker};
pub use core::DeviceCore;
pub use error::{ModelError, ModelError as DeviceError, ModelResult, PdmaResetError, PdmaRunError};
pub use manual::SyncDevice;
pub(crate) use state::{RunnerShared, publish, wake};
pub use types::{AsyncRunSnapshot, AsyncRunSummary, AsyncStallReason, AsyncStepStatus};
use worker::run_worker;
