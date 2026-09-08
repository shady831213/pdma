use hardware_interface_pipeline::{Stage, StepOutcome};

use super::{RetirementPorts, RetirementStatus, RetirementUnit};
use crate::device::ModelStepStatus;
use crate::device::context::HardwareContext;
use crate::device::flow::{ExecutionOutcome, RetirableExecution, RetirementWork};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(in crate::device) enum RetirementState {
    Work(RetirementWork),
    Retry,
    Done,
}

impl Stage<HardwareContext<'_>> for RetirementUnit {
    type Input = RetirementWork;
    type Output = ModelStepStatus;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = RetirementState;

    fn start(input: Self::Input) -> Self::State {
        RetirementState::Work(input)
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let input = std::mem::replace(state, RetirementState::Done);
        let mut ports = RetirementPorts::new(context);
        let status = match input {
            RetirementState::Done => return StepOutcome::Exhausted,
            RetirementState::Work(RetirementWork::Empty(status)) => {
                return StepOutcome::Complete(Ok(status));
            }
            RetirementState::Work(RetirementWork::FrontendFault(fault)) => {
                self.cache_frontend_fault(&mut ports, fault)
            }
            RetirementState::Retry => self.retry(&mut ports),
            RetirementState::Work(RetirementWork::Executed(outcome)) => match outcome {
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
                *state = RetirementState::Retry;
                return StepOutcome::Pending(reason);
            }
            Ok(RetirementStatus::Progressed) => Ok(ModelStepStatus::Progressed),
            Ok(RetirementStatus::FaultCached) => Ok(ModelStepStatus::FaultCached),
            Err(error) => Err(error),
        };
        StepOutcome::Complete(result)
    }
}
