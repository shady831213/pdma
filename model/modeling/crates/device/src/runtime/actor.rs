//! Tokio actor facade over one shared device core.

use super::PdmaMemoryProvisioning;
use super::backend::DeviceInteractionBackend;
use super::state::{RunnerState, summary};
use super::{
    AsyncRunSummary, DeviceCore, MmioRegisterEndpoint, ModelResult, PendingDmaResult, RunnerShared,
    run_worker,
};
use crate::memory::PdmaMemoryError;
use hardware_interface_runtime::{
    NoopRegisterObserver, ObservationGate, RegisterObserver, SharedRegisterObserver,
};
use pdma_model_contract::semantic::trace::TraceEvent;
use std::sync::Arc;

/// Cloneable host-facing handle for an async hardware runner.
#[derive(Debug, Clone)]
pub struct AsyncDeviceHandle {
    pub(crate) shared: Arc<RunnerShared>,
    pub(crate) memory: hardware_interface_runtime::SharedMemoryAccess,
}

/// Host-facing async hardware runner state.
#[derive(Debug)]
pub struct AsyncDevice {
    pub(crate) handle: AsyncDeviceHandle,
}

/// Explicit async hardware worker loop returned to callers for spawning or direct awaiting.
#[derive(Debug)]
pub struct AsyncDeviceWorker {
    shared: Arc<RunnerShared>,
}

impl AsyncDevice {
    /// Consumes one device core into the asynchronous facade and worker.
    pub fn from_core(
        core: DeviceCore,
        provisioning: PdmaMemoryProvisioning,
    ) -> Result<(Self, AsyncDeviceWorker), PdmaMemoryError> {
        Self::from_core_observed(core, provisioning, Arc::new(NoopRegisterObserver))
    }

    /// Constructs a core and runtime from one explicitly selected memory backend.
    pub fn new(
        provisioning: PdmaMemoryProvisioning,
        config: pdma_model_contract::semantic::ResourceConfig,
    ) -> (Self, AsyncDeviceWorker) {
        Self::new_observed(provisioning, config, Arc::new(NoopRegisterObserver))
    }

    /// Constructs a core and observed runtime from one selected memory backend.
    pub fn new_observed(
        provisioning: PdmaMemoryProvisioning,
        config: pdma_model_contract::semantic::ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
    ) -> (Self, AsyncDeviceWorker) {
        let core = DeviceCore::new(provisioning.memory_access(), config);
        Self::assemble(core, provisioning, observer)
    }

    /// Consumes one device core with an ordered register observer.
    pub fn from_core_observed(
        core: DeviceCore,
        provisioning: PdmaMemoryProvisioning,
        observer: Arc<dyn RegisterObserver>,
    ) -> Result<(Self, AsyncDeviceWorker), PdmaMemoryError> {
        provisioning.validate_binding(&core.memory)?;
        Ok(Self::assemble(core, provisioning, observer))
    }

    fn assemble(
        core: DeviceCore,
        provisioning: PdmaMemoryProvisioning,
        observer: Arc<dyn RegisterObserver>,
    ) -> (Self, AsyncDeviceWorker) {
        let memory = provisioning.memory_access();
        let shared = Arc::new(RunnerShared {
            backend: DeviceInteractionBackend::new(RunnerState::new(core, provisioning)),
            register_gate: ObservationGate::new(SharedRegisterObserver::new(observer)),
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
        self.shared.backend.interact(|state| summary(state)).await
    }
}

impl AsyncDeviceHandle {
    /// Returns the clone-shared exact register endpoint owned by this device core.
    pub fn register_endpoint(&self) -> MmioRegisterEndpoint {
        MmioRegisterEndpoint::new(Arc::clone(&self.shared))
    }

    /// Returns committed invocations of the sole reset runner.
    pub async fn reset_runner_calls(&self) -> usize {
        self.shared
            .backend
            .interact(|state| state.core.reset_runner_calls())
            .await
    }

    /// Requests worker shutdown.
    pub async fn request_shutdown(&self) {
        let generation = self
            .shared
            .backend
            .interact(|state| {
                state.shutdown_requested = true;
                state.generation = state.generation.saturating_add(1);
                state.generation
            })
            .await;
        self.shared.backend.publish(generation);
    }

    /// Returns the one cached DMA completion or event before bus publication.
    pub async fn pending_dma_result(&self) -> Option<PendingDmaResult> {
        self.shared
            .backend
            .interact(|state| state.core.pending_dma_result())
            .await
    }

    /// Enables deterministic trace capture.
    pub async fn enable_tracing(&self) {
        self.with_device_unit(DeviceCore::enable_tracing).await;
    }

    /// Disables trace capture without clearing existing events.
    pub async fn disable_tracing(&self) {
        self.with_device_unit(DeviceCore::disable_tracing).await;
    }

    /// Clears captured trace events and sequence ordinals.
    pub async fn clear_trace(&self) {
        self.with_device_unit(DeviceCore::clear_trace).await;
    }

    /// Returns captured trace events in deterministic sequence order.
    pub async fn trace_events(&self) -> Vec<TraceEvent> {
        self.shared
            .backend
            .interact(|state| state.core.trace_events())
            .await
    }

    /// Returns normalized frontend fetches that entered the native step core.
    pub async fn native_steps(&self) -> usize {
        self.shared
            .backend
            .interact(|state| state.core.native_steps())
            .await
    }

    /// Exports captured trace events as Chrome/Perfetto JSON.
    pub async fn trace_json(&self) -> String {
        self.shared
            .backend
            .interact(|state| state.core.trace_json())
            .await
    }

    pub(crate) async fn with_device<T: Send>(
        &self,
        f: impl FnOnce(&mut DeviceCore) -> ModelResult<T> + Send,
    ) -> ModelResult<T> {
        self.shared
            .backend
            .update(|state| (f(&mut state.core), true))
            .await
    }

    async fn with_device_unit(&self, f: impl FnOnce(&mut DeviceCore) + Send) {
        self.shared
            .backend
            .update(|state| (f(&mut state.core), true))
            .await;
    }
}
