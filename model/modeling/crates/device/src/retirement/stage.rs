use hardware_interface_pipeline::{Stage, StepOutcome};

use super::{RetirementPorts, RetirementStatus, RetirementUnit};
use crate::device::ModelStepStatus;
use crate::execution::{ExecutionOutcome, RetirableExecution};
use crate::queue::io::NormalizedFault;
use crate::scheduler::stage::QueueFeedback;
use crate::{AsyncStallReason, ModelError, ModelResult};

#[derive(Debug)]
pub(crate) enum RetirementInput {
    Empty(ModelStepStatus),
    FrontendFault(NormalizedFault),
    Executed(ExecutionOutcome),
    Retry,
}

#[derive(Debug)]
pub(crate) struct RetirementFeedback {
    pub(crate) result: ModelResult<ModelStepStatus>,
    pub(crate) runnable: Option<QueueFeedback>,
}

impl Stage<RetirementPorts<'_>> for RetirementUnit {
    type Input = RetirementInput;
    type Output = RetirementFeedback;
    type Error = std::convert::Infallible;
    type Wait = AsyncStallReason;
    type State = Option<RetirementInput>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(input) = state.take() else {
            return StepOutcome::Exhausted;
        };
        let status = match input {
            RetirementInput::Empty(status) => {
                return StepOutcome::Complete(Ok(RetirementFeedback {
                    result: Ok(status),
                    runnable: None,
                }));
            }
            RetirementInput::FrontendFault(fault) => self.cache_frontend_fault(ports, fault),
            RetirementInput::Retry => self.retry(ports),
            RetirementInput::Executed(outcome) => match outcome {
                ExecutionOutcome::Success(success) => {
                    self.retire(ports, RetirableExecution::Success(success))
                }
                ExecutionOutcome::Fault(fault) => {
                    self.retire(ports, RetirableExecution::Fault(fault))
                }
                ExecutionOutcome::InvalidCompletion(route) => ports
                    .retire_command(route.queue)
                    .and(Err(ModelError::UnknownCompletionRing(
                        route.completion_ring,
                    ))),
            },
        };
        let result = match status {
            Ok(RetirementStatus::Stalled(reason)) => {
                *state = Some(RetirementInput::Retry);
                return StepOutcome::Pending(reason);
            }
            Ok(RetirementStatus::Progressed) => Ok(ModelStepStatus::Progressed),
            Ok(RetirementStatus::FaultCached) => Ok(ModelStepStatus::FaultCached),
            Err(error) => Err(error),
        };
        StepOutcome::Complete(Ok(RetirementFeedback {
            result,
            runnable: ports.feedback.take(),
        }))
    }
}
