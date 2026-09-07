use hardware_interface_pipeline::{Stage, StepOutcome};

use super::execute::ExecutedRoute;
use super::fetch::FetchedRoute;
use super::{DeviceCore, ModelStepStatus, execute, retire};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) enum Operation {
    Terminal(Option<ModelStepStatus>),
    RetryCompletion(bool),
    RetryFault(bool),
    FrontendFault(Option<crate::queue::io::NormalizedFault>),
    Copy(execute::Copy),
    StridedCopy(execute::StridedCopy),
    BroadcastCopy(execute::BroadcastCopy),
    VectorAdd(execute::VectorAdd),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RetirementRoute {
    Terminal(ModelStepStatus),
    RetryCompletion,
    RetryFault,
    FrontendFault(crate::queue::io::NormalizedFault),
    Executed(ExecutedRoute),
}

#[derive(Debug)]
pub(super) enum Retirement {
    Terminal(Option<ModelStepStatus>),
    Success(retire::Success),
    FrontendFault(retire::FrontendFailure),
    ExecutionFault(retire::ExecutionFailure),
    InvalidCompletion(retire::InvalidCompletion),
    PendingCompletion(retire::PendingCompletion),
    PendingFault(retire::PendingFault),
}

type RouteOutcome = StepOutcome<RetirementRoute, ModelError, AsyncStallReason>;
type PipelineOutcome = StepOutcome<ModelStepStatus, ModelError, AsyncStallReason>;

impl Operation {
    pub(super) fn from_fetch(route: FetchedRoute) -> Self {
        match route {
            FetchedRoute::Terminal(status) => Self::Terminal(Some(status)),
            FetchedRoute::RetryCompletion => Self::RetryCompletion(true),
            FetchedRoute::RetryFault => Self::RetryFault(true),
            FetchedRoute::FrontendFault(fault) => Self::FrontendFault(Some(fault)),
            FetchedRoute::Copy(route, request) => Self::Copy(execute::Copy::new(route, request)),
            FetchedRoute::StridedCopy(route, request) => {
                Self::StridedCopy(execute::StridedCopy::new(route, request))
            }
            FetchedRoute::BroadcastCopy(route, request) => {
                Self::BroadcastCopy(execute::BroadcastCopy::new(route, request))
            }
            FetchedRoute::VectorAdd(route, request) => {
                Self::VectorAdd(execute::VectorAdd::new(route, request))
            }
        }
    }

    const fn map_execution(
        outcome: &StepOutcome<ExecutedRoute, ModelError, AsyncStallReason>,
    ) -> RouteOutcome {
        match outcome {
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(wait) => StepOutcome::Pending(*wait),
            StepOutcome::Complete(Ok(route)) => {
                StepOutcome::Complete(Ok(RetirementRoute::Executed(*route)))
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(*error)),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}

impl Stage<DeviceCore> for Operation {
    type Error = ModelError;
    type Output = RetirementRoute;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RouteOutcome {
        match self {
            Self::Terminal(status) => complete_once(status, RetirementRoute::Terminal),
            Self::RetryCompletion(active) => {
                complete_flag(active, RetirementRoute::RetryCompletion)
            }
            Self::RetryFault(active) => complete_flag(active, RetirementRoute::RetryFault),
            Self::FrontendFault(fault) => complete_once(fault, RetirementRoute::FrontendFault),
            Self::Copy(stage) => Self::map_execution(&stage.step(context)),
            Self::StridedCopy(stage) => Self::map_execution(&stage.step(context)),
            Self::BroadcastCopy(stage) => Self::map_execution(&stage.step(context)),
            Self::VectorAdd(stage) => Self::map_execution(&stage.step(context)),
        }
    }
}

impl Retirement {
    pub(super) const fn from_route(route: RetirementRoute) -> Self {
        match route {
            RetirementRoute::Terminal(status) => Self::Terminal(Some(status)),
            RetirementRoute::RetryCompletion => {
                Self::PendingCompletion(retire::PendingCompletion::new())
            }
            RetirementRoute::RetryFault => Self::PendingFault(retire::PendingFault::new()),
            RetirementRoute::FrontendFault(fault) => {
                Self::FrontendFault(retire::FrontendFailure::new(fault))
            }
            RetirementRoute::Executed(ExecutedRoute::Success(success)) => {
                Self::Success(retire::Success::new(success))
            }
            RetirementRoute::Executed(ExecutedRoute::Fault(fault)) => {
                Self::ExecutionFault(retire::ExecutionFailure::new(fault))
            }
            RetirementRoute::Executed(ExecutedRoute::InvalidCompletion(route)) => {
                Self::InvalidCompletion(retire::InvalidCompletion::new(route))
            }
        }
    }
}

impl Stage<DeviceCore> for Retirement {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> PipelineOutcome {
        match self {
            Self::Terminal(status) => complete_once(status, |status| status),
            Self::Success(stage) => stage.step(context),
            Self::FrontendFault(stage) => stage.step(context),
            Self::ExecutionFault(stage) => stage.step(context),
            Self::InvalidCompletion(stage) => stage.step(context),
            Self::PendingCompletion(stage) => stage.step(context),
            Self::PendingFault(stage) => stage.step(context),
        }
    }
}

fn complete_once<T, O>(
    pending: &mut Option<T>,
    map: impl FnOnce(T) -> O,
) -> StepOutcome<O, ModelError, AsyncStallReason> {
    pending.take().map_or_else(
        || StepOutcome::Exhausted,
        |value| StepOutcome::Complete(Ok(map(value))),
    )
}

fn complete_flag<T>(active: &mut bool, output: T) -> StepOutcome<T, ModelError, AsyncStallReason> {
    if !*active {
        return StepOutcome::Exhausted;
    }
    *active = false;
    StepOutcome::Complete(Ok(output))
}
