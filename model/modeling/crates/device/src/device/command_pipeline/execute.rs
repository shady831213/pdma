use hardware_interface_pipeline::{Stage, StepOutcome};

use super::super::{DeviceCore, ModelStepStatus};
use super::fetch::FetchedRoute;
use crate::execution::ExecutionOutcome;
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) enum Execute {
    Terminal(ModelStepStatus),
    RetryCompletion,
    RetryFault,
    FrontendFault(crate::queue::io::NormalizedFault),
    Copy(
        crate::execution::CommandRoute,
        pdma_model_contract::semantic::Command,
    ),
    Strided(
        crate::execution::CommandRoute,
        pdma_model_contract::semantic::Command,
    ),
    Broadcast(
        crate::execution::CommandRoute,
        pdma_model_contract::semantic::Command,
    ),
    VectorAdd(
        crate::execution::CommandRoute,
        pdma_model_contract::semantic::Command,
    ),
    Done,
}

#[derive(Debug)]
pub(super) enum ExecutedRoute {
    Terminal(ModelStepStatus),
    RetryCompletion,
    RetryFault,
    FrontendFault(crate::queue::io::NormalizedFault),
    Success(crate::execution::SuccessfulExecution),
    Fault(crate::execution::ExecutionFault),
    InvalidCompletion(crate::execution::CommandRoute),
}

impl From<FetchedRoute> for Execute {
    fn from(route: FetchedRoute) -> Self {
        match route {
            FetchedRoute::Terminal(status) => Self::Terminal(status),
            FetchedRoute::RetryCompletion => Self::RetryCompletion,
            FetchedRoute::RetryFault => Self::RetryFault,
            FetchedRoute::FrontendFault(fault) => Self::FrontendFault(fault),
            FetchedRoute::Copy(route, command) => Self::Copy(route, command),
            FetchedRoute::Strided(route, command) => Self::Strided(route, command),
            FetchedRoute::Broadcast(route, command) => Self::Broadcast(route, command),
            FetchedRoute::VectorAdd(route, command) => Self::VectorAdd(route, command),
        }
    }
}

impl Execute {
    fn pending(
        context: &mut DeviceCore,
        route: crate::execution::CommandRoute,
        reason: AsyncStallReason,
    ) -> Result<StepOutcome<ExecutedRoute, ModelError, AsyncStallReason>, ModelError> {
        if context.queues.has_pending(route.queue)? {
            context.scheduler.enqueue(route.queue);
        }
        context.trace.stall(Some(route.queue), stall_name(reason));
        Ok(StepOutcome::Pending(reason))
    }
}

impl Stage<DeviceCore> for Execute {
    type Error = ModelError;
    type Output = ExecutedRoute;
    type Wait = AsyncStallReason;

    fn step(
        &mut self,
        context: &mut DeviceCore,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let state = std::mem::replace(self, Self::Done);
        match state {
            Self::Terminal(status) => StepOutcome::Complete(Ok(ExecutedRoute::Terminal(status))),
            Self::RetryCompletion => StepOutcome::Complete(Ok(ExecutedRoute::RetryCompletion)),
            Self::RetryFault => StepOutcome::Complete(Ok(ExecutedRoute::RetryFault)),
            Self::FrontendFault(fault) => {
                StepOutcome::Complete(Ok(ExecutedRoute::FrontendFault(fault)))
            }
            Self::Copy(route, command) => self.run_command(context, route, command, Self::Copy),
            Self::Strided(route, command) => {
                self.run_command(context, route, command, Self::Strided)
            }
            Self::Broadcast(route, command) => {
                self.run_command(context, route, command, Self::Broadcast)
            }
            Self::VectorAdd(route, command) => {
                self.run_command(context, route, command, Self::VectorAdd)
            }
            Self::Done => StepOutcome::Exhausted,
        }
    }
}

impl Execute {
    fn run_command(
        &mut self,
        context: &mut DeviceCore,
        route: crate::execution::CommandRoute,
        command: pdma_model_contract::semantic::Command,
        restore: fn(crate::execution::CommandRoute, pdma_model_contract::semantic::Command) -> Self,
    ) -> StepOutcome<ExecutedRoute, ModelError, AsyncStallReason> {
        if !context.execution.can_execute(&command) {
            return match Self::pending(context, route, AsyncStallReason::ExecutionResourcePressure)
            {
                Ok(outcome) => {
                    *self = restore(route, command);
                    outcome
                }
                Err(error) => StepOutcome::Complete(Err(error)),
            };
        }
        match context
            .execution
            .execute(&context.dma, route, command.clone())
        {
            Ok(outcome) => StepOutcome::Complete(Ok(match outcome {
                ExecutionOutcome::Success(success) => ExecutedRoute::Success(success),
                ExecutionOutcome::Fault(fault) => ExecutedRoute::Fault(fault),
                ExecutionOutcome::InvalidCompletion(route) => {
                    ExecutedRoute::InvalidCompletion(route)
                }
            })),
            Err(reason) => {
                *self = restore(route, command);
                StepOutcome::Pending(reason)
            }
        }
    }
}

const fn stall_name(reason: AsyncStallReason) -> &'static str {
    match reason {
        AsyncStallReason::ExecutionResourcePressure => "ExecutionResourcePressure",
        AsyncStallReason::CompletionPublicationPressure => "CompletionPublicationPressure",
        AsyncStallReason::EventPublicationPressure => "EventPublicationPressure",
        AsyncStallReason::FaultRetirementPendingRetry => "FaultRetirementPendingRetry",
    }
}
