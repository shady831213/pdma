//! Shared asynchronous device state and wake publication.

use super::{AsyncRunSnapshot, AsyncRunSummary};
use crate::AsyncStepStatus;
pub(crate) use crate::device::DeviceCore;
use tokio::sync::{Mutex, watch};

#[derive(Debug)]
pub(crate) struct RunnerShared {
    pub(crate) state: Mutex<DeviceCore>,
    pub(crate) changed: watch::Sender<usize>,
}

pub(crate) const fn snapshot(state: &DeviceCore) -> AsyncRunSnapshot {
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
    state: &DeviceCore,
    last_status: AsyncStepStatus,
) -> AsyncRunSnapshot {
    AsyncRunSnapshot {
        last_status,
        ..snapshot(state)
    }
}

pub(crate) const fn summary(state: &DeviceCore) -> AsyncRunSummary {
    AsyncRunSummary {
        steps: state.steps,
        errors: state.errors,
        stalls: state.stalls,
        last_status: state.last_status,
    }
}

pub(crate) const fn wake(state: &mut DeviceCore) {
    state.mark_changed();
}

pub(crate) fn publish(shared: &RunnerShared, generation: usize) {
    shared.changed.send_replace(generation);
}
