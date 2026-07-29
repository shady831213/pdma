//! Tokio-backed asynchronous hardware runner.

mod backend;
mod bus;
mod irq;
mod manual;
mod native;
mod queue_api;
mod reset;
mod state;
mod types;
mod wait;
mod worker;

use crate::dma_space::{DmaSpaceError, DmaStage};
use backend::DeviceModel;
pub use bus::{BusControlError, DmaWrite, QueueStatus, SubmissionAcceptance};
pub use manual::SyncDevice;
use pdma_model_contract::ResourceConfig;
use pdma_model_contract::abi::{DmaAddress, DmaRegion};
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::ids::BufferId;
use pdma_model_contract::trace::TraceEvent;
use state::{RunnerShared, RunnerState, publish, summary, wake};
use std::sync::Arc;
pub use types::{AsyncRunSnapshot, AsyncRunSummary, AsyncStallReason, AsyncStepStatus};
use worker::run_worker;

/// Cloneable host-facing handle for an async hardware runner.
#[derive(Debug, Clone)]
pub struct AsyncDeviceHandle {
    shared: Arc<RunnerShared>,
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
    /// Creates an async hardware runner backed by the queue-port device model.
    pub fn queue_port() -> (Self, AsyncDeviceWorker) {
        Self::queue_port_with_resource_config(ResourceConfig::default())
    }

    /// Creates an async hardware runner backed by the queue-port device model with `config` credits.
    pub fn queue_port_with_resource_config(config: ResourceConfig) -> (Self, AsyncDeviceWorker) {
        if config == ResourceConfig::default() {
            Self::from_backend(DeviceModel::queue_port())
        } else {
            Self::from_backend(DeviceModel::queue_port_with_resource_config(config))
        }
    }

    fn from_backend(backend: DeviceModel) -> (Self, AsyncDeviceWorker) {
        let (changed, _) = tokio::sync::watch::channel(0);
        let shared = Arc::new(RunnerShared {
            state: tokio::sync::Mutex::new(RunnerState::new(backend)),
            changed,
        });
        let worker = AsyncDeviceWorker {
            shared: Arc::clone(&shared),
        };
        (
            Self {
                handle: AsyncDeviceHandle { shared },
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

    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> ModelResult<BufferId> {
        self.allocate_dma_region(len).await.map(DmaRegion::object)
    }

    pub(crate) async fn allocate_dma_region(&self, len: usize) -> ModelResult<DmaRegion> {
        let (region, generation) = {
            let mut state = self.shared.state.lock().await;
            let region = u64::try_from(len)
                .map_err(|_| pdma_model_contract::error::ModelError::DmaAllocationExhausted)
                .and_then(|length| {
                    state
                        .dma_space
                        .allocate(length)
                        .map_err(|_| pdma_model_contract::error::ModelError::DmaAllocationExhausted)
                });
            if region.is_ok() {
                wake(&mut state);
            }
            (region, state.generation)
        };
        if region.is_ok() {
            publish(&self.shared, generation);
        }
        region
    }

    /// Writes bytes into a checked device buffer range.
    pub async fn write_buffer(&self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.write_object(id, offset, data);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    /// Reads bytes from a checked device buffer range.
    pub async fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .read_object(id, offset, len)
    }

    /// Proposes DMA regions without changing mappings, bytes, or allocator generation.
    pub async fn stage_dma_regions(&self, lengths: &[u64]) -> Result<DmaStage, DmaSpaceError> {
        self.shared.state.lock().await.dma_space.stage(lengths)
    }

    /// Atomically commits every region in a stage.
    pub async fn commit_dma_stage(
        &self,
        proposal: DmaStage,
    ) -> Result<Vec<DmaRegion>, DmaSpaceError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.commit(proposal);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    /// Reads one checked IOVA extent.
    pub async fn read_iova(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> Result<Vec<u8>, DmaSpaceError> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .read_iova(address, length)
    }

    /// Writes one checked IOVA extent.
    pub async fn write_iova(&self, address: DmaAddress, data: &[u8]) -> Result<(), DmaSpaceError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.write_iova(address, data);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    /// Resolves a semantic object to its committed DMA region.
    pub async fn region_for_object(&self, object: BufferId) -> Option<DmaRegion> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .region_for_object(object)
    }

    /// Returns the committed DMA allocator generation.
    pub async fn dma_generation(&self) -> u64 {
        self.shared.state.lock().await.dma_space.generation()
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
