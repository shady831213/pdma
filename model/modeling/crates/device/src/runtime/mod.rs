//! Tokio-backed asynchronous hardware runner.

mod actor;
mod batch;
mod dma;
mod endpoint;
mod error;
mod global_api;
pub mod irq;
mod manual;
mod memory;
mod mmio;
mod queue;
mod reset;
mod ring;
pub mod state;
mod test_support;
mod types;
mod wait;
mod worker;

use crate::AsyncStepStatus;
pub(crate) use crate::abi::ring::{PdmaControlError, PendingDmaResult};
pub use crate::device::DeviceCore;
use crate::dma::PdmaMemoryError;
pub use actor::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker};
pub use endpoint::MmioRegisterEndpoint;
pub use error::{ModelError, ModelError as DeviceError, ModelResult, PdmaResetError, PdmaRunError};
pub use manual::SyncDevice;
pub(crate) use state::{RunnerShared, publish, wake};
pub use types::{AsyncRunSnapshot, AsyncRunSummary};
use worker::run_worker;
