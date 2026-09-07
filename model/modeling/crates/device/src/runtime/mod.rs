//! Tokio-backed asynchronous hardware runner.

mod access;
mod actor;
mod backend;
#[doc(hidden)]
pub(crate) mod state;
mod types;
mod wait;
mod worker;

use crate::AsyncStepStatus;
use crate::ModelResult;
use crate::device::DeviceCore;
pub(crate) use crate::queue::exact::PendingDmaResult;
pub use access::MmioRegisterEndpoint;
pub use actor::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker};
pub(crate) use backend::DeviceWait;
pub(crate) use state::RunnerShared;
pub use types::{AsyncRunSnapshot, AsyncRunSummary};
use worker::run_worker;
