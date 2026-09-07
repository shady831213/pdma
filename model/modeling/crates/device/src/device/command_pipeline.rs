use hardware_interface_pipeline::{Stage, StepOutcome, Then};

use super::hardware_context::HardwareContext;
use super::{DeviceCore, ModelStepStatus};
use crate::execution::ExecutionUnit;
use crate::queue::issue::IssueView;
use crate::retirement::RetirementPorts;
use crate::retirement::stage::RetirementInput;
use crate::{AsyncStallReason, ModelError};

type Dispatch = Then<IssueView<'static>, ExecutionUnit>;
type DispatchState = <Dispatch as Stage<HardwareContext<'static>>>::State;

#[derive(Debug)]
enum CommandState {
    Dispatch(DispatchState),
    Retiring(Option<RetirementInput>),
}

#[derive(Debug)]
pub(super) struct PipelineSlot {
    state: Option<CommandState>,
}

impl PipelineSlot {
    pub(super) const fn new() -> Self {
        Self { state: None }
    }

    pub(super) fn step(
        &mut self,
        core: &mut DeviceCore,
    ) -> StepOutcome<ModelStepStatus, ModelError, AsyncStallReason> {
        let state = self.state.get_or_insert_with(|| {
            if core.retirement.pending().is_some() {
                CommandState::Retiring(Some(RetirementInput::Retry))
            } else {
                CommandState::Dispatch(<Dispatch as Stage<HardwareContext<'_>>>::start(()))
            }
        });
        let outcome = match state {
            CommandState::Dispatch(dispatch) => {
                let mut stages = Then::new(
                    IssueView {
                        scheduler: &mut core.scheduler,
                        queues: &mut core.queues,
                    },
                    &mut core.execution,
                );
                let mut context = HardwareContext {
                    dma: &core.dma,
                    trace: &mut core.trace,
                    fetches: &mut core.native_fetches,
                };
                match stages.step(&mut context, dispatch) {
                    StepOutcome::Progressed => StepOutcome::Progressed,
                    StepOutcome::Pending(reason) => StepOutcome::Pending(reason),
                    StepOutcome::Complete(Ok(input)) => {
                        *state = CommandState::Retiring(Some(input));
                        StepOutcome::Progressed
                    }
                    StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
                    StepOutcome::Exhausted => StepOutcome::Exhausted,
                }
            }
            CommandState::Retiring(retirement) => {
                let mut ports = RetirementPorts::new(
                    &mut core.queues,
                    &core.dma,
                    &mut core.irq,
                    &mut core.trace,
                    core.execution.resources(),
                );
                match core.retirement.step(&mut ports, retirement) {
                    StepOutcome::Progressed => StepOutcome::Progressed,
                    StepOutcome::Pending(reason) => StepOutcome::Pending(reason),
                    StepOutcome::Complete(Ok(feedback)) => {
                        if let Some(runnable) = feedback.runnable {
                            core.scheduler.accept_feedback(runnable);
                        }
                        StepOutcome::Complete(feedback.result)
                    }
                    StepOutcome::Complete(Err(error)) => match error {},
                    StepOutcome::Exhausted => StepOutcome::Exhausted,
                }
            }
        };
        if matches!(outcome, StepOutcome::Complete(_) | StepOutcome::Exhausted) {
            self.clear();
        }
        outcome
    }

    pub(super) fn clear(&mut self) {
        self.state = None;
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.state.is_none()
    }
}

impl Stage<()> for DeviceCore {
    type Input = ();
    type Output = crate::HardwareStepStatus;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = ();

    fn start((): Self::Input) -> Self::State {}

    fn step(
        &mut self,
        (): &mut (),
        (): &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.step_pipeline()
    }
}

#[cfg(test)]
mod tests;
