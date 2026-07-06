use super::AsyncStepStatus;
use super::backend::ModelStepStatus;
use super::state::{RunnerShared, RunnerState, publish};
use std::sync::Arc;

pub(super) async fn run_worker(shared: Arc<RunnerShared>) {
    while run_one(&shared).await {}
}

async fn run_one(shared: &RunnerShared) -> bool {
    let outcome = {
        let mut state = shared.state.lock().await;
        if state.shutdown_requested {
            return publish_and_continue(shared, state.generation, false);
        }
        match state.backend.step() {
            Ok(ModelStepStatus::Progressed) => {
                state.steps = state.steps.saturating_add(1);
                state.last_status = AsyncStepStatus::Progressed;
                state.generation = state.generation.saturating_add(1);
                WorkerOutcome::Continue(state.generation)
            }
            Ok(ModelStepStatus::Idle) => {
                state.last_status = AsyncStepStatus::Idle;
                WorkerOutcome::Wait(wait_for_wake(shared, &mut state))
            }
            Ok(ModelStepStatus::Stalled(reason)) => {
                state.stalls = state.stalls.saturating_add(1);
                state.last_status = AsyncStepStatus::Stalled(reason);
                WorkerOutcome::Wait(wait_for_wake(shared, &mut state))
            }
            Err(_) => {
                state.errors = state.errors.saturating_add(1);
                state.generation = state.generation.saturating_add(1);
                WorkerOutcome::Continue(state.generation)
            }
        }
    };
    match outcome {
        WorkerOutcome::Continue(generation) => publish_and_continue(shared, generation, true),
        WorkerOutcome::Wait(wait) => {
            publish(shared, wait.generation);
            wait_for_generation_change(shared, wait).await;
            true
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum WorkerOutcome {
    Continue(usize),
    Wait(WaitState),
}

#[derive(Debug, Clone, Copy)]
struct WaitState {
    generation: usize,
    wake_generation: usize,
}

fn wait_for_wake(shared: &RunnerShared, state: &mut RunnerState) -> WaitState {
    let wake_generation = state.wake_generation;
    state.generation = state.generation.saturating_add(1);
    publish(shared, state.generation);
    WaitState {
        generation: state.generation,
        wake_generation,
    }
}

async fn wait_for_generation_change(shared: &RunnerShared, wait: WaitState) {
    let mut changes = shared.changed.subscribe();
    loop {
        let should_wait = {
            let state = shared.state.lock().await;
            state.wake_generation == wait.wake_generation && !state.shutdown_requested
        };
        if !should_wait {
            return;
        }
        if *changes.borrow_and_update() != wait.generation {
            continue;
        }
        if changes.changed().await.is_err() {
            return;
        }
    }
}

fn publish_and_continue(shared: &RunnerShared, generation: usize, keep_running: bool) -> bool {
    publish(shared, generation);
    keep_running
}
