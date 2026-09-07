use std::convert::Infallible;

use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, Operation, Stage, StepOutcome};

use super::AsyncStepStatus;
use super::backend::DeviceWait;
use super::state::{RunnerShared, RunnerState};
use crate::device::drive::ModelStepStatus;

pub(super) async fn run_worker(shared: &RunnerShared) {
    let executor = AsyncExecutor::new(&shared.backend);
    let mut service = DeviceService::new();
    let mut operation = Operation::new(());
    match executor.run(&mut service, &mut operation).await {
        Ok(()) | Err(AsyncRunError::Exhausted) => {}
        Err(AsyncRunError::BackendInteraction(error) | AsyncRunError::BackendWait(error)) => {
            match error {}
        }
        Err(AsyncRunError::Stage(error)) => match error {},
    }
}

#[derive(Debug)]
struct DeviceService;

impl DeviceService {
    const fn new() -> Self {
        Self
    }
}

impl Stage<RunnerState> for DeviceService {
    type Input = ();
    type Error = Infallible;
    type Output = ();
    type Wait = DeviceWait;
    type State = bool;

    fn start((): Self::Input) -> Self::State {
        true
    }

    fn step(
        &mut self,
        state: &mut RunnerState,
        active: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        if !*active {
            return StepOutcome::Exhausted;
        }
        if state.shutdown_requested {
            state.core.clear_command_pipeline();
            *active = false;
            state.worker_publication = Some(state.generation);
            return StepOutcome::Complete(Ok(()));
        }
        let outcome = Stage::step(&mut state.core, &mut (), &mut ());
        match outcome {
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(reason)
            | StepOutcome::Complete(Ok(ModelStepStatus::Stalled(reason))) => {
                state.stalls = state.stalls.saturating_add(1);
                state.last_status = AsyncStepStatus::Stalled(reason);
                StepOutcome::Pending(wait_for_wake(state))
            }
            StepOutcome::Complete(Ok(ModelStepStatus::Progressed)) => {
                state.steps = state.steps.saturating_add(1);
                progressed(state)
            }
            StepOutcome::Complete(Ok(ModelStepStatus::FaultCached)) => progressed(state),
            StepOutcome::Complete(Ok(ModelStepStatus::Idle)) => {
                state.last_status = AsyncStepStatus::Idle;
                StepOutcome::Pending(wait_for_wake(state))
            }
            StepOutcome::Complete(Err(_)) => {
                state.errors = state.errors.saturating_add(1);
                state.generation = state.generation.saturating_add(1);
                state.worker_publication = Some(state.generation);
                StepOutcome::Progressed
            }
            StepOutcome::Exhausted => StepOutcome::Exhausted,
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
