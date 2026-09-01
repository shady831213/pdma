//! Tokio-backed asynchronous hardware runner.

mod actor;
mod batch;
mod dma;
mod endpoint;
mod error;
mod global_api;
mod irq;
mod memory;
mod mmio;
mod queue;
mod reset;
mod ring;
#[doc(hidden)]
pub mod state;
mod types;
mod wait;
mod worker;

use crate::AsyncStepStatus;
pub(crate) use crate::abi::ring::PendingDmaResult;
pub use crate::device::DeviceCore;
pub use actor::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker};
pub use endpoint::MmioRegisterEndpoint;
pub use error::{ModelError, ModelError as DeviceError, ModelResult};
pub(crate) use state::{RunnerShared, publish, wake};
pub use types::{AsyncRunSnapshot, AsyncRunSummary};
use worker::run_worker;
