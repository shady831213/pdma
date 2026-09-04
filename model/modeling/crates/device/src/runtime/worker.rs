use super::AsyncStepStatus;
use super::backend::DeviceWait;
use super::state::{RunnerShared, RunnerState};
use crate::device::drive::{ModelStepStatus, PipelineStep};

pub(super) async fn run_worker(shared: &RunnerShared) {
    while run_one(shared).await {}
}

async fn run_one(shared: &RunnerShared) -> bool {
    let outcome = shared
        .backend
        .interact(|state| {
            if state.shutdown_requested {
                state.core.clear_command_pipeline();
                return WorkerOutcome::Stop(state.generation);
            }
            match state.core.step_pipeline() {
                PipelineStep::InternalProgress => WorkerOutcome::ContinueUnpublished,
                PipelineStep::Terminal(Ok(ModelStepStatus::Progressed)) => {
                    state.steps = state.steps.saturating_add(1);
                    state.last_status = AsyncStepStatus::Progressed;
                    state.generation = state.generation.saturating_add(1);
                    WorkerOutcome::Continue(state.generation)
                }
                PipelineStep::Terminal(Ok(ModelStepStatus::FaultCached)) => {
                    state.last_status = AsyncStepStatus::Progressed;
                    state.generation = state.generation.saturating_add(1);
                    WorkerOutcome::Continue(state.generation)
                }
                PipelineStep::Terminal(Ok(ModelStepStatus::Idle)) => {
                    state.last_status = AsyncStepStatus::Idle;
                    WorkerOutcome::Wait(wait_for_wake(state))
                }
                PipelineStep::Terminal(Ok(ModelStepStatus::Stalled(reason)))
                | PipelineStep::Pending(reason) => {
                    state.stalls = state.stalls.saturating_add(1);
                    state.last_status = AsyncStepStatus::Stalled(reason);
                    WorkerOutcome::Wait(wait_for_wake(state))
                }
                PipelineStep::Terminal(Err(_)) => {
                    state.errors = state.errors.saturating_add(1);
                    state.generation = state.generation.saturating_add(1);
                    WorkerOutcome::Continue(state.generation)
                }
            }
        })
        .await;
    match outcome {
        WorkerOutcome::ContinueUnpublished => true,
        WorkerOutcome::Stop(generation) => publish_and_continue(shared, generation, false),
        WorkerOutcome::Continue(generation) => publish_and_continue(shared, generation, true),
        WorkerOutcome::Wait(wait) => {
            shared.backend.publish(wait_generation(wait));
            shared.backend.wait_for_change(wait).await;
            true
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum WorkerOutcome {
    ContinueUnpublished,
    Stop(usize),
    Continue(usize),
    Wait(DeviceWait),
}

const fn wait_for_wake(state: &mut RunnerState) -> DeviceWait {
    let wake_generation = state.wake_generation;
    state.generation = state.generation.saturating_add(1);
    DeviceWait::wake(state.generation, wake_generation)
}

const fn wait_generation(wait: DeviceWait) -> usize {
    wait.generation_value()
}

fn publish_and_continue(shared: &RunnerShared, generation: usize, keep_running: bool) -> bool {
    shared.backend.publish(generation);
    keep_running
}

#[cfg(test)]
mod tests;
