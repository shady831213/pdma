//! Thread-backed synchronous hardware runner.

mod state;

use crate::PdmaDevice;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use state::{RunnerShared, RunnerState, lock_state, snapshot, summary, wait_state, wake};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

/// Join failure for a sync hardware runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncRunJoinError {
    /// The worker thread panicked before join completed.
    WorkerPanicked,
}

/// Snapshot of runner-observable hardware progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncRunSnapshot {
    /// Commands retired successfully by the worker loop.
    pub steps: usize,
    /// Model errors observed by the worker loop.
    pub errors: usize,
    /// Stall statuses observed by the worker loop.
    pub stalls: usize,
    /// Whether shutdown has been requested.
    pub shutdown_requested: bool,
    generation: usize,
}

/// Final runner summary returned by `join`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncRunSummary {
    /// Commands retired successfully by the worker loop.
    pub steps: usize,
    /// Model errors observed by the worker loop.
    pub errors: usize,
    /// Stall statuses observed by the worker loop.
    pub stalls: usize,
}

/// Cloneable host-facing handle for a sync hardware runner.
#[derive(Debug, Clone)]
pub struct SyncDeviceHandle {
    shared: Arc<RunnerShared>,
}

/// Owns the sync hardware worker thread.
#[derive(Debug)]
pub struct SyncDeviceRunner {
    handle: SyncDeviceHandle,
    worker: Option<JoinHandle<()>>,
}

impl PdmaDevice {
    /// Spawns a sync hardware runner that advances this device until shutdown.
    ///
    /// # Errors
    /// Returns the OS thread-spawn error if the worker thread cannot be created.
    pub fn spawn_sync_runner(self) -> io::Result<SyncDeviceRunner> {
        let shared = Arc::new(RunnerShared {
            state: Mutex::new(RunnerState::new(self)),
            changed: std::sync::Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("pdma-sync-runner".to_owned())
            .spawn(move || run_worker(&worker_shared))?;
        Ok(SyncDeviceRunner {
            handle: SyncDeviceHandle { shared },
            worker: Some(worker),
        })
    }
}

impl SyncDeviceRunner {
    /// Returns a cloneable handle used by host software.
    pub fn handle(&self) -> SyncDeviceHandle {
        self.handle.clone()
    }

    /// Returns the current runner snapshot.
    pub fn snapshot(&self) -> SyncRunSnapshot {
        self.handle.snapshot()
    }

    /// Waits until the runner records any change after the supplied snapshot.
    pub fn wait_for_change_after(&self, snapshot: SyncRunSnapshot) -> SyncRunSnapshot {
        self.handle.wait_for_change_after(snapshot)
    }

    /// Waits until the runner records more progress than `steps`.
    pub fn wait_for_progress_after(&self, steps: usize) -> SyncRunSnapshot {
        self.handle.wait_for_progress_after(steps)
    }

    /// Waits until the runner records more errors than `errors`.
    pub fn wait_for_error_after(&self, errors: usize) -> SyncRunSnapshot {
        self.handle.wait_for_error_after(errors)
    }

    /// Requests worker shutdown.
    pub fn request_shutdown(&self) {
        self.handle.request_shutdown();
    }

    /// Requests shutdown and joins the worker thread.
    pub fn join(mut self) -> Result<SyncRunSummary, SyncRunJoinError> {
        self.request_shutdown();
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                return Err(SyncRunJoinError::WorkerPanicked);
            }
        }
        Ok(summary(&lock_state(&self.handle.shared)))
    }
}

impl SyncDeviceHandle {
    /// Requests worker shutdown.
    pub fn request_shutdown(&self) {
        {
            let mut state = lock_state(&self.shared);
            state.shutdown_requested = true;
            state.generation = state.generation.saturating_add(1);
        }
        self.shared.changed.notify_all();
    }

    /// Returns the current runner snapshot.
    pub fn snapshot(&self) -> SyncRunSnapshot {
        snapshot(&lock_state(&self.shared))
    }

    /// Waits until the runner records any change after the supplied snapshot.
    pub fn wait_for_change_after(&self, previous: SyncRunSnapshot) -> SyncRunSnapshot {
        let mut state = lock_state(&self.shared);
        while state.generation == previous.generation && !state.shutdown_requested {
            state = wait_state(&self.shared, state);
        }
        let snapshot = snapshot(&state);
        drop(state);
        snapshot
    }

    /// Waits until the runner records more progress than `steps`.
    pub fn wait_for_progress_after(&self, steps: usize) -> SyncRunSnapshot {
        let mut state = lock_state(&self.shared);
        while state.steps <= steps && !state.shutdown_requested {
            state = wait_state(&self.shared, state);
        }
        let snapshot = snapshot(&state);
        drop(state);
        snapshot
    }

    /// Waits until the runner records more errors than `errors`.
    pub fn wait_for_error_after(&self, errors: usize) -> SyncRunSnapshot {
        let mut state = lock_state(&self.shared);
        while state.errors <= errors && !state.shutdown_requested {
            state = wait_state(&self.shared, state);
        }
        let snapshot = snapshot(&state);
        drop(state);
        snapshot
    }

    /// Provisions one command queue and one bound completion ring.
    pub fn create_queue(&self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.with_device(|device| device.create_queue(config))
    }

    /// Allocates a device-visible memory buffer.
    pub fn allocate_buffer(&self, len: usize) -> BufferId {
        let id = {
            let mut state = lock_state(&self.shared);
            let id = state.device.allocate_buffer(len);
            wake(&mut state);
            drop(state);
            id
        };
        self.shared.changed.notify_all();
        id
    }

    /// Writes bytes into a checked device buffer range.
    pub fn write_buffer(&self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        self.with_device(|device| device.write_buffer(id, offset, data))
    }

    /// Reads bytes from a checked device buffer range.
    pub fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        let state = lock_state(&self.shared);
        state.device.read_buffer(id, offset, len)
    }

    /// Pushes one command into a queue's command ring.
    pub fn submit_command(&self, queue_id: QueueId, command: ExternalCommand) -> ModelResult<()> {
        self.with_device(|device| device.submit_command(queue_id, command))
    }

    /// Captures a software doorbell for one queue.
    pub fn ring_doorbell(&self, queue_id: QueueId) -> ModelResult<()> {
        self.with_device(|device| device.ring_doorbell(queue_id))
    }

    /// Pops one completion from a completion ring.
    pub fn poll_completion(
        &self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.with_device(|device| device.poll_completion(completion_ring_id))
    }

    /// Pops one event for a specific queue without consuming other queues' events.
    pub fn poll_queue_event(&self, queue_id: QueueId) -> ModelResult<Option<EventRecord>> {
        self.with_device(|device| device.poll_queue_event(queue_id))
    }

    fn with_device<T>(&self, f: impl FnOnce(&mut PdmaDevice) -> ModelResult<T>) -> ModelResult<T> {
        let result = {
            let mut state = lock_state(&self.shared);
            let result = f(&mut state.device);
            wake(&mut state);
            drop(state);
            result
        };
        self.shared.changed.notify_all();
        result
    }
}

fn run_worker(shared: &RunnerShared) {
    while run_one(shared) {}
}

fn run_one(shared: &RunnerShared) -> bool {
    let mut state = lock_state(shared);
    if state.shutdown_requested {
        drop(state);
        shared.changed.notify_all();
        return false;
    }
    match state.device.step_with_status() {
        Ok(crate::StepStatus::Progressed) => {
            state.steps = state.steps.saturating_add(1);
            state.generation = state.generation.saturating_add(1);
        }
        Ok(crate::StepStatus::Idle) => {
            state = wait_for_wake(shared, state);
        }
        Ok(crate::StepStatus::Stalled(_)) => {
            state.stalls = state.stalls.saturating_add(1);
            state = wait_for_wake(shared, state);
        }
        Err(_) => {
            state.errors = state.errors.saturating_add(1);
            state.generation = state.generation.saturating_add(1);
        }
    }
    drop(state);
    shared.changed.notify_all();
    true
}

fn wait_for_wake<'a>(
    shared: &'a RunnerShared,
    mut state: MutexGuard<'a, RunnerState>,
) -> MutexGuard<'a, RunnerState> {
    let wake_generation = state.wake_generation;
    state.generation = state.generation.saturating_add(1);
    shared.changed.notify_all();
    while state.wake_generation == wake_generation && !state.shutdown_requested {
        state = wait_state(shared, state);
    }
    state
}
