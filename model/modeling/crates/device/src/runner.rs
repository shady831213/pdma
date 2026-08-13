//! Tokio-backed asynchronous hardware runner.

mod backend;
mod bus;
mod core;
mod global_api;
mod irq;
mod manual;
mod mmio;
mod native;
mod queue_api;
mod register_endpoint;
mod reset;
mod state;
mod types;
mod wait;
mod worker;

use crate::ModelResult;
use crate::dma_space::{PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage};
use backend::DeviceModel;
pub use bus::{
    BusControlError, DmaBatchContext, DmaWrite, MmioBatchError, QueueBootstrapBatch,
    QueueResponseBytes, QueueStatus, SubmissionAcceptance, SubmissionBatch,
};
pub use core::DeviceCore;
use hardware_interface_runtime::MemoryAddress;
pub use manual::SyncDevice;
use pdma_model_contract::bridge::PdmaMemoryRegion;
use pdma_model_contract::ids::BufferId;
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
    /// Returns the clone-shared exact register endpoint owned by this device core.
    pub fn register_endpoint(&self) -> MmioRegisterEndpoint {
        MmioRegisterEndpoint::new(Arc::clone(&self.shared))
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

    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> ModelResult<BufferId> {
        self.allocate_memory_region(len)
            .await
            .map(PdmaMemoryRegion::object)
    }

    /// Allocates one raw device-visible DMA region.
    pub async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        let (region, generation) = {
            let mut state = self.shared.state.lock().await;
            let region = u64::try_from(len)
                .map_err(|_| crate::ModelError::DmaAllocationExhausted)
                .and_then(|length| {
                    state
                        .dma_space
                        .allocate(length)
                        .map_err(|_| crate::ModelError::DmaAllocationExhausted)
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
    pub async fn stage_memory_regions(
        &self,
        lengths: &[u64],
    ) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        self.shared.state.lock().await.dma_space.stage(lengths)
    }

    /// Atomically commits every region in a stage.
    pub async fn commit_memory_stage(
        &self,
        proposal: PdmaMemoryStage,
    ) -> Result<Vec<PdmaMemoryRegion>, PdmaMemoryError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.commit(proposal);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }

    /// Reads one checked mapped-memory extent.
    pub async fn read_memory(
        &self,
        address: impl Into<MemoryAddress>,
        length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .read_memory(address.into(), length)
    }

    /// Writes one checked mapped-memory extent.
    pub async fn write_memory(
        &self,
        address: impl Into<MemoryAddress>,
        data: &[u8],
    ) -> Result<(), PdmaMemoryError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = state.dma_space.write_memory(address.into(), data);
            if result.is_ok() {
                wake(&mut state);
            }
            (result, state.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Resolves a semantic object to its committed DMA region.
    pub async fn region_for_object(&self, object: BufferId) -> Option<PdmaMemoryRegion> {
        self.shared
            .state
            .lock()
            .await
            .dma_space
            .region_for_object(object)
    }

    /// Returns the committed DMA allocator generation.
    pub async fn memory_generation(&self) -> u64 {
        self.shared.state.lock().await.dma_space.generation()
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
