use hardware_interface_pipeline::{Stage, StepOutcome};

use super::super::{DeviceCore, ModelStepStatus};
use super::execute::ExecutedRoute;
use crate::execution::RetirableExecution;
use crate::retirement::{RetirementPorts, RetirementStatus};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) enum Retire {
    Terminal(ModelStepStatus),
    RetryCompletion,
    RetryFault,
    FrontendFault(crate::queue::io::NormalizedFault),
    Success(crate::execution::SuccessfulExecution),
    ExecutionFault(crate::execution::ExecutionFault),
    InvalidCompletion(crate::execution::CommandRoute),
    Done,
}

impl From<ExecutedRoute> for Retire {
    fn from(route: ExecutedRoute) -> Self {
        match route {
            ExecutedRoute::Terminal(status) => Self::Terminal(status),
            ExecutedRoute::RetryCompletion => Self::RetryCompletion,
            ExecutedRoute::RetryFault => Self::RetryFault,
            ExecutedRoute::FrontendFault(fault) => Self::FrontendFault(fault),
            ExecutedRoute::Success(success) => Self::Success(success),
            ExecutedRoute::Fault(fault) => Self::ExecutionFault(fault),
            ExecutedRoute::InvalidCompletion(route) => Self::InvalidCompletion(route),
        }
    }
}

impl Retire {
    const fn finish(
        &mut self,
        status: Result<RetirementStatus, ModelError>,
        retry: Self,
    ) -> StepOutcome<ModelStepStatus, ModelError, AsyncStallReason> {
        match status {
            Ok(RetirementStatus::Progressed) => {
                StepOutcome::Complete(Ok(ModelStepStatus::Progressed))
            }
            Ok(RetirementStatus::FaultCached) => {
                StepOutcome::Complete(Ok(ModelStepStatus::FaultCached))
            }
            Ok(RetirementStatus::Stalled(reason)) => {
                *self = retry;
                StepOutcome::Pending(reason)
            }
            Err(error) => StepOutcome::Complete(Err(error)),
        }
    }
}

impl Stage<DeviceCore> for Retire {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(
        &mut self,
        context: &mut DeviceCore,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let state = std::mem::replace(self, Self::Done);
        match state {
            Self::Terminal(status) => StepOutcome::Complete(Ok(status)),
            Self::RetryCompletion => {
                let status = retry(context);
                self.finish(status, Self::RetryCompletion)
            }
            Self::RetryFault => {
                let status = retry(context);
                self.finish(status, Self::RetryFault)
            }
            Self::FrontendFault(fault) => {
                let status = cache_frontend_fault(context, fault);
                self.finish(status, Self::RetryFault)
            }
            Self::Success(success) => {
                let status = retire(context, RetirableExecution::Success(success));
                self.finish(status, Self::RetryCompletion)
            }
            Self::ExecutionFault(fault) => {
                let status = retire(context, RetirableExecution::Fault(fault));
                self.finish(status, Self::RetryFault)
            }
            Self::InvalidCompletion(route) => {
                let result = context.queues.consume(route.queue).and_then(|()| {
                    if context.queues.has_pending(route.queue)? {
                        context.scheduler.enqueue(route.queue);
                    }
                    Err(ModelError::UnknownCompletionRing(route.completion_ring))
                });
                StepOutcome::Complete(result)
            }
            Self::Done => StepOutcome::Exhausted,
        }
    }
}

fn retry(context: &mut DeviceCore) -> Result<RetirementStatus, ModelError> {
    let resources = context.execution.resources();
    let mut ports = RetirementPorts::new(
        &mut context.queues,
        &context.dma,
        &mut context.scheduler,
        &mut context.irq,
        &mut context.trace,
        resources,
    );
    context.retirement.retry(&mut ports)
}

fn retire(
    context: &mut DeviceCore,
    outcome: RetirableExecution,
) -> Result<RetirementStatus, ModelError> {
    let resources = context.execution.resources();
    let mut ports = RetirementPorts::new(
        &mut context.queues,
        &context.dma,
        &mut context.scheduler,
        &mut context.irq,
        &mut context.trace,
        resources,
    );
    context.retirement.retire(&mut ports, outcome)
}

fn cache_frontend_fault(
    context: &mut DeviceCore,
    fault: crate::queue::io::NormalizedFault,
) -> Result<RetirementStatus, ModelError> {
    let resources = context.execution.resources();
    let mut ports = RetirementPorts::new(
        &mut context.queues,
        &context.dma,
        &mut context.scheduler,
        &mut context.irq,
        &mut context.trace,
        resources,
    );
    context.retirement.cache_frontend_fault(&mut ports, fault)
}
