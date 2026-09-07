use std::convert::Infallible;

use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, Stage, StepOutcome};

use super::AsyncStepStatus;
use super::backend::DeviceWait;
use super::state::{RunnerShared, RunnerState};
use crate::device::drive::{ModelStepStatus, PipelineStep};

pub(super) async fn run_worker(shared: &RunnerShared) {
    let executor = AsyncExecutor::new(&shared.backend);
    let mut pipeline = WorkerPipeline::new();
    match executor.run(&mut pipeline).await {
        Ok(()) | Err(AsyncRunError::Exhausted) => {}
        Err(AsyncRunError::BackendInteraction(error) | AsyncRunError::BackendWait(error)) => {
            match error {}
        }
        Err(AsyncRunError::Stage(error)) => match error {},
    }
}

#[derive(Debug)]
struct WorkerPipeline {
    active: bool,
}

impl WorkerPipeline {
    const fn new() -> Self {
        Self { active: true }
    }
}

impl Stage<RunnerState> for WorkerPipeline {
    type Error = Infallible;
    type Output = ();
    type Wait = DeviceWait;

    fn step(
        &mut self,
        state: &mut RunnerState,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        if !self.active {
            return StepOutcome::Exhausted;
        }
        if state.shutdown_requested {
            state.core.clear_command_pipeline();
            self.active = false;
            state.worker_publication = Some(state.generation);
            return StepOutcome::Complete(Ok(()));
        }
        match state.core.step_pipeline() {
            PipelineStep::InternalProgress => StepOutcome::Progressed,
            PipelineStep::Terminal(Ok(ModelStepStatus::Progressed)) => {
                state.steps = state.steps.saturating_add(1);
                progressed(state)
            }
            PipelineStep::Terminal(Ok(ModelStepStatus::FaultCached)) => progressed(state),
            PipelineStep::Terminal(Ok(ModelStepStatus::Idle)) => {
                state.last_status = AsyncStepStatus::Idle;
                StepOutcome::Pending(wait_for_wake(state))
            }
            PipelineStep::Terminal(Ok(ModelStepStatus::Stalled(reason)))
            | PipelineStep::Pending(reason) => {
                state.stalls = state.stalls.saturating_add(1);
                state.last_status = AsyncStepStatus::Stalled(reason);
                StepOutcome::Pending(wait_for_wake(state))
            }
            PipelineStep::Terminal(Err(_)) => {
                state.errors = state.errors.saturating_add(1);
                state.generation = state.generation.saturating_add(1);
                state.worker_publication = Some(state.generation);
                StepOutcome::Progressed
            }
        }
    }
}

const fn progressed(state: &mut RunnerState) -> StepOutcome<(), Infallible, DeviceWait> {
    state.last_status = AsyncStepStatus::Progressed;
    state.generation = state.generation.saturating_add(1);
    state.worker_publication = Some(state.generation);
    StepOutcome::Progressed
}

const fn wait_for_wake(state: &mut RunnerState) -> DeviceWait {
    let wake_generation = state.wake_generation;
    state.generation = state.generation.saturating_add(1);
    state.worker_publication = Some(state.generation);
    DeviceWait::wake(state.generation, wake_generation)
}

#[cfg(test)]
mod tests;
