//! Shared asynchronous device state and wake publication.

use super::{AsyncRunSnapshot, AsyncRunSummary};
use crate::AsyncStepStatus;
pub(crate) use crate::device::DeviceCore;
use hardware_interface_runtime::ObservationGate;
use tokio::sync::{Mutex, watch};

#[derive(Debug)]
pub(crate) struct RunnerState {
    pub(crate) core: DeviceCore,
    pub(crate) steps: usize,
    pub(crate) errors: usize,
    pub(crate) stalls: usize,
    pub(crate) last_status: AsyncStepStatus,
    pub(crate) shutdown_requested: bool,
    pub(crate) generation: usize,
    pub(crate) wake_generation: usize,
}

impl RunnerState {
    pub(crate) const fn new(core: DeviceCore) -> Self {
        Self {
            core,
            steps: 0,
            errors: 0,
            stalls: 0,
            last_status: AsyncStepStatus::Idle,
            shutdown_requested: false,
            generation: 0,
            wake_generation: 0,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RunnerShared {
    pub(crate) state: Mutex<RunnerState>,
    pub(crate) changed: watch::Sender<usize>,
    pub(crate) register_gate: ObservationGate,
}

pub(crate) const fn snapshot(state: &RunnerState) -> AsyncRunSnapshot {
    AsyncRunSnapshot {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
        last_status: state.last_status,
        shutdown_requested: state.shutdown_requested,
        generation: state.generation,
    }
}

pub(crate) const fn snapshot_with_status(
    state: &RunnerState,
    last_status: AsyncStepStatus,
) -> AsyncRunSnapshot {
    AsyncRunSnapshot {
        last_status,
        ..snapshot(state)
    }
}

pub(crate) const fn summary(state: &RunnerState) -> AsyncRunSummary {
    AsyncRunSummary {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
        last_status: state.last_status,
    }
}

pub(crate) const fn wake(state: &mut RunnerState) {
    state.wake_generation = state.wake_generation.saturating_add(1);
    state.generation = state.generation.saturating_add(1);
}

pub(crate) fn publish(shared: &RunnerShared, generation: usize) {
    shared.changed.send_replace(generation);
}
