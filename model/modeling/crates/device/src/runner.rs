//! Tokio-backed asynchronous hardware runner.

mod backend;
mod bus;
mod core;
mod global_api;
mod irq;
mod manual;
mod memory;
mod mmio;
mod native;
mod queue_api;
mod register_endpoint;
mod reset;
mod state;
#[path = "runner/test_support.rs"]
mod test_support;
mod types;
mod wait;
mod worker;

use crate::ModelResult;
use crate::dma_space::{PdmaMemoryError, PdmaMemoryMappings};
use backend::DeviceModel;
pub use bus::{
    BusControlError, DmaBatchContext, DmaWrite, MmioBatchError, PdmaBatchError, PdmaControlError,
    QueueBootstrapBatch, QueueResponseBytes, QueueStatus, SubmissionAcceptance, SubmissionBatch,
};
pub use core::{DeviceCore, DeviceStateSnapshot, ResetPartitionSnapshot};
pub use manual::SyncDevice;
use pdma_model_contract::trace::TraceEvent;
pub use register_endpoint::{MmioRegisterEndpoint, MmioRegisterError};
use state::{RunnerShared, publish, summary, wake};
use std::sync::Arc;
pub use types::{
    AsyncRunSnapshot, AsyncRunSummary, AsyncStallReason, AsyncStepStatus, PendingDmaResult,
};
use worker::run_worker;

/// Cloneable host-facing handle for an async hardware runner.
#[derive(Debug, Clone)]
pub struct AsyncDeviceHandle {
    shared: Arc<RunnerShared>,
    memory: Arc<hardware_interface_runtime::SharedMemory>,
}

/// Host-facing async hardware runner state.
#[derive(Debug)]
pub struct AsyncDevice {
    handle: AsyncDeviceHandle,
}

/// Explicit async hardware worker loop returned to callers for spawning or direct awaiting.
#[derive(Debug)]
pub struct AsyncDeviceWorker {
    shared: Arc<RunnerShared>,
}

impl AsyncDevice {
    /// Consumes one device core into the asynchronous facade and worker.
    pub fn from_core(core: DeviceCore) -> (Self, AsyncDeviceWorker) {
        let memory = core.dma_space.shared_memory();
        let (changed, _) = tokio::sync::watch::channel(0);
        let shared = Arc::new(RunnerShared {
            state: tokio::sync::Mutex::new(core),
            changed,
        });
        let worker = AsyncDeviceWorker {
            shared: Arc::clone(&shared),
        };
        (
            Self {
                handle: AsyncDeviceHandle { shared, memory },
            },
            worker,
        )
    }

    /// Returns a cloneable handle used by host software.
    pub fn handle(&self) -> AsyncDeviceHandle {
        self.handle.clone()
    }

    /// Requests worker shutdown.
    pub async fn request_shutdown(&self) {
        self.handle.request_shutdown().await;
    }
}

impl AsyncDeviceWorker {
    /// Runs the async hardware worker loop until shutdown is requested.
    pub async fn run(self) -> AsyncRunSummary {
        run_worker(&self.shared).await;
        let state = self.shared.state.lock().await;
        summary(&state)
    }
}

impl AsyncDeviceHandle {
    /// Returns the clone-shared exact register endpoint owned by this device core.
    pub fn register_endpoint(&self) -> MmioRegisterEndpoint {
        MmioRegisterEndpoint::new(Arc::clone(&self.shared))
    }

    /// Returns committed invocations of the sole reset runner.
    pub async fn reset_runner_calls(&self) -> usize {
        self.shared.state.lock().await.reset_runner_calls()
    }

    /// Requests worker shutdown.
    pub async fn request_shutdown(&self) {
        let generation = {
            let mut state = self.shared.state.lock().await;
            state.shutdown_requested = true;
            state.generation = state.generation.saturating_add(1);
            state.generation
        };
        publish(&self.shared, generation);
    }

    /// Returns the one cached DMA completion or event before bus publication.
    pub async fn pending_dma_result(&self) -> Option<PendingDmaResult> {
        self.shared.state.lock().await.backend.pending_dma_result()
    }

    /// Enables deterministic trace capture.
    pub async fn enable_tracing(&self) {
        self.with_backend_unit(DeviceModel::enable_tracing).await;
    }

    /// Disables trace capture without clearing existing events.
    pub async fn disable_tracing(&self) {
        self.with_backend_unit(DeviceModel::disable_tracing).await;
    }

    /// Clears captured trace events and sequence ordinals.
    pub async fn clear_trace(&self) {
        self.with_backend_unit(DeviceModel::clear_trace).await;
    }

    /// Returns captured trace events in deterministic sequence order.
    pub async fn trace_events(&self) -> Vec<TraceEvent> {
        self.shared.state.lock().await.backend.trace_events()
    }

    /// Returns normalized frontend fetches that entered the native step core.
    pub async fn native_steps(&self) -> usize {
        self.shared.state.lock().await.backend.native_steps()
    }

    /// Exports captured trace events as Chrome/Perfetto JSON.
    pub async fn trace_json(&self) -> String {
        self.shared.state.lock().await.backend.trace_json()
    }

    async fn with_backend<T>(
        &self,
        f: impl FnOnce(&mut DeviceModel) -> ModelResult<T>,
    ) -> ModelResult<T> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = f(&mut state.backend);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    async fn with_backend_unit(&self, f: impl FnOnce(&mut DeviceModel)) {
        let generation = {
            let mut state = self.shared.state.lock().await;
            f(&mut state.backend);
            wake(&mut state);
            state.generation
        };
        publish(&self.shared, generation);
    }
}
