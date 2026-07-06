//! Tokio-backed asynchronous hardware runner.

mod backend;
mod manual;
mod native;
mod state;
mod types;
mod wait;
mod worker;

use backend::RunnerBackend;
pub use manual::SyncDevice;
use pdma_model_contract::ResourceConfig;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::trace::TraceEvent;
use state::{RunnerShared, RunnerState, publish, summary, wake};
use std::sync::Arc;
use tokio::task::JoinHandle;
pub use types::{
    AsyncRunJoinError, AsyncRunSnapshot, AsyncRunSummary, AsyncStallReason, AsyncStepStatus,
};
use worker::run_worker;

/// Cloneable host-facing handle for an async hardware runner.
#[derive(Debug, Clone)]
pub struct AsyncDeviceHandle {
    shared: Arc<RunnerShared>,
}

/// Owns the async hardware worker task.
#[derive(Debug)]
pub struct AsyncDevice {
    handle: AsyncDeviceHandle,
    worker: JoinHandle<()>,
}

impl AsyncDevice {
    /// Spawns an async hardware runner backed by the native async linear-copy device slice.
    pub fn spawn_native() -> Self {
        Self::spawn_native_with_resource_config(ResourceConfig::default())
    }

    /// Spawns an async hardware runner backed by the native async device with `config` credits.
    pub fn spawn_native_with_resource_config(config: ResourceConfig) -> Self {
        if config == ResourceConfig::default() {
            Self::spawn_backend(RunnerBackend::native())
        } else {
            Self::spawn_backend(RunnerBackend::native_with_resource_config(config))
        }
    }

    fn spawn_backend(backend: RunnerBackend) -> Self {
        let (changed, _) = tokio::sync::watch::channel(0);
        let shared = Arc::new(RunnerShared {
            state: tokio::sync::Mutex::new(RunnerState::new(backend)),
            changed,
        });
        let worker_shared = Arc::clone(&shared);
        let worker = tokio::spawn(async move { run_worker(worker_shared).await });
        Self {
            handle: AsyncDeviceHandle { shared },
            worker,
        }
    }

    /// Returns a cloneable handle used by host software.
    pub fn handle(&self) -> AsyncDeviceHandle {
        self.handle.clone()
    }

    /// Requests worker shutdown.
    pub async fn request_shutdown(&self) {
        self.handle.request_shutdown().await;
    }

    /// Requests shutdown and joins the worker task.
    pub async fn join(self) -> Result<AsyncRunSummary, AsyncRunJoinError> {
        self.request_shutdown().await;
        match self.worker.await {
            Ok(()) => {
                let state = self.handle.shared.state.lock().await;
                Ok(summary(&state))
            }
            Err(error) if error.is_panic() => Err(AsyncRunJoinError::WorkerPanicked),
            Err(_) => Err(AsyncRunJoinError::WorkerCancelled),
        }
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

    /// Provisions one command queue and one bound completion ring.
    pub async fn create_queue(&self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.with_backend(|backend| backend.create_queue(config))
            .await
    }

    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> BufferId {
        let (id, generation) = {
            let mut state = self.shared.state.lock().await;
            let id = state.backend.allocate_buffer(len);
            wake(&mut state);
            (id, state.generation)
        };
        publish(&self.shared, generation);
        id
    }

    /// Writes bytes into a checked device buffer range.
    pub async fn write_buffer(&self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.with_backend(|backend| backend.write_buffer(id, offset, data))
            .await
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
            .backend
            .read_buffer(id, offset, len)
    }

    /// Pushes one command into a queue's command ring.
    pub async fn submit_command(
        &self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.with_backend(|backend| backend.submit_command(queue_id, command))
            .await
    }

    /// Captures a software doorbell for one queue.
    pub async fn ring_doorbell(&self, queue_id: QueueId) -> ModelResult<()> {
        self.with_backend(|backend| backend.ring_doorbell(queue_id))
            .await
    }

    /// Pops one completion from a completion ring.
    pub async fn poll_completion(
        &self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.with_backend(|backend| backend.poll_completion(completion_ring_id))
            .await
    }

    /// Pops one event for a queue-local event path.
    pub async fn poll_queue_event(&self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        self.with_backend(|backend| backend.poll_queue_event(queue_id))
            .await
    }

    /// Enables deterministic trace capture.
    pub async fn enable_tracing(&self) {
        self.with_backend_unit(RunnerBackend::enable_tracing).await;
    }

    /// Disables trace capture without clearing existing events.
    pub async fn disable_tracing(&self) {
        self.with_backend_unit(RunnerBackend::disable_tracing).await;
    }

    /// Clears captured trace events and sequence ordinals.
    pub async fn clear_trace(&self) {
        self.with_backend_unit(RunnerBackend::clear_trace).await;
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
        f: impl FnOnce(&mut RunnerBackend) -> ModelResult<T>,
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

    async fn with_backend_unit(&self, f: impl FnOnce(&mut RunnerBackend)) {
        let generation = {
            let mut state = self.shared.state.lock().await;
            f(&mut state.backend);
            wake(&mut state);
            state.generation
        };
        publish(&self.shared, generation);
    }
}
