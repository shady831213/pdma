//! Tokio-backed asynchronous hardware runner.

mod actor;
mod backend;
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
pub(crate) mod state;
mod types;
mod wait;
mod worker;

use crate::AsyncStepStatus;
pub use crate::device::DeviceCore;
pub(crate) use crate::queue::exact::PendingDmaResult;
pub use actor::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker};
pub(crate) use backend::DeviceWait;
pub use endpoint::MmioRegisterEndpoint;
pub use error::{ModelError, ModelError as DeviceError, ModelResult};
pub(crate) use state::{RunnerShared, wake};
pub use types::{AsyncRunSnapshot, AsyncRunSummary};
use worker::run_worker;
