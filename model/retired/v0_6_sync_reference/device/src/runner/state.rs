use super::{SyncRunSnapshot, SyncRunSummary};
use crate::PdmaDevice;
use std::sync::{Condvar, Mutex, MutexGuard};

#[derive(Debug)]
pub(super) struct RunnerShared {
    pub(super) state: Mutex<RunnerState>,
    pub(super) changed: Condvar,
}

#[derive(Debug)]
pub(super) struct RunnerState {
    pub(super) device: PdmaDevice,
    pub(super) steps: usize,
    pub(super) errors: usize,
    pub(super) stalls: usize,
    pub(super) generation: usize,
    pub(super) wake_generation: usize,
    pub(super) shutdown_requested: bool,
}

impl RunnerState {
    pub(super) const fn new(device: PdmaDevice) -> Self {
        Self {
            device,
            steps: 0,
            errors: 0,
            stalls: 0,
            generation: 0,
            wake_generation: 0,
            shutdown_requested: false,
        }
    }
}

pub(super) const fn snapshot(state: &RunnerState) -> SyncRunSnapshot {
    SyncRunSnapshot {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
        shutdown_requested: state.shutdown_requested,
        generation: state.generation,
    }
}

pub(super) const fn summary(state: &RunnerState) -> SyncRunSummary {
    SyncRunSummary {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
    }
}

pub(super) const fn wake(state: &mut RunnerState) {
    state.wake_generation = state.wake_generation.saturating_add(1);
    state.generation = state.generation.saturating_add(1);
}

pub(super) fn lock_state(shared: &RunnerShared) -> MutexGuard<'_, RunnerState> {
    match shared.state.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(super) fn wait_state<'a>(
    shared: &'a RunnerShared,
    state: MutexGuard<'a, RunnerState>,
) -> MutexGuard<'a, RunnerState> {
    match shared.changed.wait(state) {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    }
}
