use super::{AsyncRunSnapshot, AsyncRunSummary};
use crate::runner::backend::RunnerBackend;
use tokio::sync::{Mutex, watch};

#[derive(Debug)]
pub(super) struct RunnerShared {
    pub(super) state: Mutex<RunnerState>,
    pub(super) changed: watch::Sender<usize>,
}

#[derive(Debug)]
pub(super) struct RunnerState {
    pub(super) backend: RunnerBackend,
    pub(super) steps: usize,
    pub(super) errors: usize,
    pub(super) stalls: usize,
    pub(super) generation: usize,
    pub(super) wake_generation: usize,
    pub(super) shutdown_requested: bool,
}

impl RunnerState {
    pub(super) const fn new(backend: RunnerBackend) -> Self {
        Self {
            backend,
            steps: 0,
            errors: 0,
            stalls: 0,
            generation: 0,
            wake_generation: 0,
            shutdown_requested: false,
        }
    }
}

pub(super) const fn snapshot(state: &RunnerState) -> AsyncRunSnapshot {
    AsyncRunSnapshot {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
        shutdown_requested: state.shutdown_requested,
        generation: state.generation,
    }
}

pub(super) const fn summary(state: &RunnerState) -> AsyncRunSummary {
    AsyncRunSummary {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
    }
}

pub(super) const fn wake(state: &mut RunnerState) {
    state.wake_generation = state.wake_generation.saturating_add(1);
    state.generation = state.generation.saturating_add(1);
}

pub(super) fn publish(shared: &RunnerShared, generation: usize) {
    shared.changed.send_replace(generation);
}
