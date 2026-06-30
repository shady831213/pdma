//! Tokio-backed asynchronous hardware runner.

mod state;
mod types;
mod worker;

use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_device::PdmaDevice;
use state::{RunnerShared, RunnerState, publish, snapshot, summary, wake};
use std::sync::Arc;
use tokio::task::JoinHandle;
pub use types::{AsyncRunJoinError, AsyncRunSnapshot, AsyncRunSummary};
use worker::run_worker;

/// Cloneable host-facing handle for an async hardware runner.
#[derive(Debug, Clone)]
pub struct AsyncDeviceHandle {
    shared: Arc<RunnerShared>,
}

/// Owns the async hardware worker task.
#[derive(Debug)]
pub struct AsyncDeviceRunner {
    handle: AsyncDeviceHandle,
    worker: JoinHandle<()>,
}

impl AsyncDeviceRunner {
    /// Spawns an async hardware runner that advances `device` until shutdown.
    pub fn spawn(device: PdmaDevice) -> Self {
        let (changed, _) = tokio::sync::watch::channel(0);
        let shared = Arc::new(RunnerShared {
            state: tokio::sync::Mutex::new(RunnerState::new(device)),
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

    /// Returns the current runner snapshot.
    pub async fn snapshot(&self) -> AsyncRunSnapshot {
        self.handle.snapshot().await
    }

    /// Waits until the runner records any change after the supplied snapshot.
    pub async fn wait_for_change_after(&self, snapshot: AsyncRunSnapshot) -> AsyncRunSnapshot {
        self.handle.wait_for_change_after(snapshot).await
    }

    /// Waits until the runner records more progress than `steps`.
    pub async fn wait_for_progress_after(&self, steps: usize) -> AsyncRunSnapshot {
        self.handle.wait_for_progress_after(steps).await
    }

    /// Waits until the runner records more errors than `errors`.
    pub async fn wait_for_error_after(&self, errors: usize) -> AsyncRunSnapshot {
        self.handle.wait_for_error_after(errors).await
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

    /// Returns the current runner snapshot.
    pub async fn snapshot(&self) -> AsyncRunSnapshot {
        let state = self.shared.state.lock().await;
        snapshot(&state)
    }

    /// Waits until the runner records any change after the supplied snapshot.
    pub async fn wait_for_change_after(&self, previous: AsyncRunSnapshot) -> AsyncRunSnapshot {
        self.wait_until(|state| state.generation != previous.generation || state.shutdown_requested)
            .await
    }

    /// Waits until the runner records more progress than `steps`.
    pub async fn wait_for_progress_after(&self, steps: usize) -> AsyncRunSnapshot {
        self.wait_until(|state| state.steps > steps || state.shutdown_requested)
            .await
    }

    /// Waits until the runner records more errors than `errors`.
    pub async fn wait_for_error_after(&self, errors: usize) -> AsyncRunSnapshot {
        self.wait_until(|state| state.errors > errors || state.shutdown_requested)
            .await
    }

    /// Provisions one command queue and one bound completion ring.
    pub async fn create_queue(&self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.with_device(|device| device.create_queue(config)).await
    }

    /// Allocates a device-visible memory buffer.
    pub async fn allocate_buffer(&self, len: usize) -> BufferId {
        let (id, generation) = {
            let mut state = self.shared.state.lock().await;
            let id = state.device.allocate_buffer(len);
            wake(&mut state);
            (id, state.generation)
        };
        publish(&self.shared, generation);
        id
    }

    /// Writes bytes into a checked device buffer range.
    pub async fn write_buffer(&self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.with_device(|device| device.write_buffer(id, offset, data))
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
            .device
            .read_buffer(id, offset, len)
    }

    /// Pushes one command into a queue's command ring.
    pub async fn submit_command(
        &self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.with_device(|device| device.submit_command(queue_id, command))
            .await
    }

    /// Captures a software doorbell for one queue.
    pub async fn ring_doorbell(&self, queue_id: QueueId) -> ModelResult<()> {
        self.with_device(|device| device.ring_doorbell(queue_id))
            .await
    }

    /// Pops one completion from a completion ring.
    pub async fn poll_completion(
        &self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.with_device(|device| device.poll_completion(completion_ring_id))
            .await
    }

    async fn wait_until(&self, predicate: impl Fn(&RunnerState) -> bool) -> AsyncRunSnapshot {
        let mut changes = self.shared.changed.subscribe();
        loop {
            let current = {
                let state = self.shared.state.lock().await;
                if predicate(&state) {
                    return snapshot(&state);
                }
                state.generation
            };
            if *changes.borrow_and_update() != current {
                continue;
            }
            if changes.changed().await.is_err() {
                return self.snapshot().await;
            }
        }
    }

    async fn with_device<T>(
        &self,
        f: impl FnOnce(&mut PdmaDevice) -> ModelResult<T>,
    ) -> ModelResult<T> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = f(&mut state.device);
            wake(&mut state);
            (result, state.generation)
        };
        publish(&self.shared, generation);
        result
    }
}
