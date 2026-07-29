use super::{AsyncRunSnapshot, AsyncRunSummary, AsyncStepStatus};
use crate::dma_space::DmaSpace;
use crate::runner::backend::DeviceModel;
use crate::runner::bus::control::BusControlState;
use tokio::sync::{Mutex, watch};

#[derive(Debug)]
pub(super) struct RunnerShared {
    pub(super) state: Mutex<RunnerState>,
    pub(super) changed: watch::Sender<usize>,
}

#[derive(Debug)]
pub(super) struct RunnerState {
    pub(super) backend: DeviceModel,
    pub(super) dma_space: DmaSpace,
    pub(super) bus_control: BusControlState,
    pub(super) steps: usize,
    pub(super) errors: usize,
    pub(super) stalls: usize,
    pub(super) last_status: AsyncStepStatus,
    pub(super) generation: usize,
    pub(super) wake_generation: usize,
    pub(super) shutdown_requested: bool,
}

impl RunnerState {
    pub(super) const fn new(backend: DeviceModel) -> Self {
        Self {
            backend,
            dma_space: DmaSpace::new(),
            bus_control: BusControlState::new(),
            steps: 0,
            errors: 0,
            stalls: 0,
            last_status: AsyncStepStatus::Idle,
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
        last_status: state.last_status,
        shutdown_requested: state.shutdown_requested,
        generation: state.generation,
    }
}

pub(super) const fn snapshot_with_status(
    state: &RunnerState,
    last_status: AsyncStepStatus,
) -> AsyncRunSnapshot {
    AsyncRunSnapshot {
        last_status,
        ..snapshot(state)
    }
}

pub(super) const fn summary(state: &RunnerState) -> AsyncRunSummary {
    AsyncRunSummary {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
        last_status: state.last_status,
    }
}

pub(super) const fn wake(state: &mut RunnerState) {
    state.wake_generation = state.wake_generation.saturating_add(1);
    state.generation = state.generation.saturating_add(1);
}

pub(super) fn publish(shared: &RunnerShared, generation: usize) {
    shared.changed.send_replace(generation);
}
