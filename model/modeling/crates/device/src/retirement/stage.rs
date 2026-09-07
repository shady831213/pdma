use hardware_interface_pipeline::{Stage, StepOutcome};

use super::{RetirementPorts, RetirementStatus, RetirementUnit};
use crate::device::ModelStepStatus;
use crate::device::hardware_context::HardwareContext;
use crate::execution::{ExecutionOutcome, RetirableExecution};
use crate::queue::io::NormalizedFault;
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(crate) enum RetirementInput {
    Empty(ModelStepStatus),
    FrontendFault(NormalizedFault),
    Executed(ExecutionOutcome),
    Retry,
}

impl Stage<HardwareContext<'_>> for RetirementUnit {
    type Input = RetirementInput;
    type Output = ModelStepStatus;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = Option<RetirementInput>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(input) = state.take() else {
            return StepOutcome::Exhausted;
        };
        let mut ports = RetirementPorts::new(context);
        let status = match input {
            RetirementInput::Empty(status) => {
                return StepOutcome::Complete(Ok(status));
            }
            RetirementInput::FrontendFault(fault) => self.cache_frontend_fault(&mut ports, fault),
            RetirementInput::Retry => self.retry(&mut ports),
            RetirementInput::Executed(outcome) => match outcome {
                ExecutionOutcome::Success(success) => {
                    self.retire(&mut ports, RetirableExecution::Success(success))
                }
                ExecutionOutcome::Fault(fault) => {
                    self.retire(&mut ports, RetirableExecution::Fault(fault))
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
        StepOutcome::Complete(result)
    }
}
