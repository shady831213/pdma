mod execute;
mod fetch;
mod retire;

#[cfg(test)]
mod tests;

use hardware_interface_pipeline::{RunOutcome, RunResult, Stage, StepExecutor, StepOutcome};

use self::execute::ExecutedRoute;
use self::fetch::{FetchedRoute, ProbeFetch};
use super::{DeviceCore, ModelStepStatus};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
enum PdmaCommandPipeline {
    ProbeFetch(ProbeFetch),
    Copy(execute::Copy),
    StridedCopy(execute::StridedCopy),
    BroadcastCopy(execute::BroadcastCopy),
    VectorAdd(execute::VectorAdd),
    SuccessRetirement(retire::Success),
    FrontendFaultRetirement(retire::FrontendFailure),
    ExecutionFaultRetirement(retire::ExecutionFailure),
    InvalidCompletionRetirement(retire::InvalidCompletion),
    PendingCompletionRetry(retire::PendingCompletion),
    PendingFaultRetry(retire::PendingFault),
    Done,
}

impl PdmaCommandPipeline {
    #[cfg(test)]
    const fn new() -> Self {
        Self::ProbeFetch(ProbeFetch::new())
    }

    const fn accepted() -> Self {
        Self::ProbeFetch(ProbeFetch::accepted())
    }

    fn from_fetch(route: FetchedRoute) -> Self {
        match route {
            FetchedRoute::Terminal(_) => Self::Done,
            FetchedRoute::RetryCompletion => {
                Self::PendingCompletionRetry(retire::PendingCompletion::new())
            }
            FetchedRoute::RetryFault => Self::PendingFaultRetry(retire::PendingFault::new()),
            FetchedRoute::FrontendFault(fault) => {
                Self::FrontendFaultRetirement(retire::FrontendFailure::new(fault))
            }
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

    const fn from_execution(route: ExecutedRoute) -> Self {
        match route {
            ExecutedRoute::Success(success) => {
                Self::SuccessRetirement(retire::Success::new(success))
            }
            ExecutedRoute::Fault(fault) => {
                Self::ExecutionFaultRetirement(retire::ExecutionFailure::new(fault))
            }
            ExecutedRoute::InvalidCompletion(route) => {
                Self::InvalidCompletionRetirement(retire::InvalidCompletion::new(route))
            }
        }
    }

    fn advance_fetch(
        &mut self,
        outcome: StepOutcome<FetchedRoute, ModelError, AsyncStallReason>,
        stage: ProbeFetch,
    ) -> PipelineOutcome {
        match outcome {
            StepOutcome::Progressed => {
                *self = Self::ProbeFetch(stage);
                StepOutcome::Progressed
            }
            StepOutcome::Pending(wait) => {
                *self = Self::ProbeFetch(stage);
                StepOutcome::Pending(wait)
            }
            StepOutcome::Complete(Ok(FetchedRoute::Terminal(status))) => {
                StepOutcome::Complete(Ok(status))
            }
            StepOutcome::Complete(Ok(route)) => {
                *self = Self::from_fetch(route);
                StepOutcome::Progressed
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }

    fn advance_execution(
        &mut self,
        outcome: &StepOutcome<ExecutedRoute, ModelError, AsyncStallReason>,
        pending: Self,
    ) -> PipelineOutcome {
        match outcome {
            StepOutcome::Progressed => {
                *self = pending;
                StepOutcome::Progressed
            }
            StepOutcome::Pending(wait) => {
                *self = pending;
                StepOutcome::Pending(*wait)
            }
            StepOutcome::Complete(Ok(route)) => {
                *self = Self::from_execution(*route);
                StepOutcome::Progressed
            }
            StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(*error)),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }

    fn advance_retirement(&mut self, outcome: &PipelineOutcome, pending: Self) -> PipelineOutcome {
        match outcome {
            StepOutcome::Pending(wait) => {
                *self = pending;
                StepOutcome::Pending(*wait)
            }
            StepOutcome::Progressed => {
                *self = pending;
                StepOutcome::Progressed
            }
            StepOutcome::Complete(result) => StepOutcome::Complete(*result),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}

type PipelineOutcome = StepOutcome<ModelStepStatus, ModelError, AsyncStallReason>;

#[derive(Debug)]
pub(super) struct PipelineSlot {
    pipeline: Option<PdmaCommandPipeline>,
}

impl PipelineSlot {
    pub(super) const fn new() -> Self {
        Self { pipeline: None }
    }

    pub(super) fn step(
        &mut self,
        context: &mut DeviceCore,
    ) -> RunResult<ModelStepStatus, ModelError, AsyncStallReason> {
        let mut pipeline = self
            .pipeline
            .take()
            .unwrap_or_else(PdmaCommandPipeline::accepted);
        let outcome = StepExecutor::new().step(&mut pipeline, context);
        if matches!(outcome, Ok(RunOutcome::Progressed | RunOutcome::Pending(_))) {
            self.pipeline = Some(pipeline);
        }
        outcome
    }

    pub(super) fn clear(&mut self) {
        self.pipeline = None;
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.pipeline.is_none()
    }
}

impl Stage<DeviceCore> for PdmaCommandPipeline {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> PipelineOutcome {
        let phase = std::mem::replace(self, Self::Done);
        match phase {
            Self::ProbeFetch(mut stage) => {
                let outcome = stage.step(context);
                self.advance_fetch(outcome, stage)
            }
            Self::Copy(mut stage) => {
                let outcome = stage.step(context);
                self.advance_execution(&outcome, Self::Copy(stage))
            }
            Self::StridedCopy(mut stage) => {
                let outcome = stage.step(context);
                self.advance_execution(&outcome, Self::StridedCopy(stage))
            }
            Self::BroadcastCopy(mut stage) => {
                let outcome = stage.step(context);
                self.advance_execution(&outcome, Self::BroadcastCopy(stage))
            }
            Self::VectorAdd(mut stage) => {
                let outcome = stage.step(context);
                self.advance_execution(&outcome, Self::VectorAdd(stage))
            }
            Self::SuccessRetirement(mut stage) => {
                let outcome = stage.step(context);
                self.advance_retirement(
                    &outcome,
                    Self::PendingCompletionRetry(retire::PendingCompletion::new()),
                )
            }
            Self::FrontendFaultRetirement(mut stage) => {
                let outcome = stage.step(context);
                self.advance_retirement(
                    &outcome,
                    Self::PendingFaultRetry(retire::PendingFault::new()),
                )
            }
            Self::ExecutionFaultRetirement(mut stage) => {
                let outcome = stage.step(context);
                self.advance_retirement(
                    &outcome,
                    Self::PendingFaultRetry(retire::PendingFault::new()),
                )
            }
            Self::InvalidCompletionRetirement(mut stage) => {
                let outcome = stage.step(context);
                self.advance_retirement(&outcome, Self::InvalidCompletionRetirement(stage))
            }
            Self::PendingCompletionRetry(mut stage) => {
                let outcome = stage.step(context);
                self.advance_retirement(&outcome, Self::PendingCompletionRetry(stage))
            }
            Self::PendingFaultRetry(mut stage) => {
                let outcome = stage.step(context);
                self.advance_retirement(&outcome, Self::PendingFaultRetry(stage))
            }
            Self::Done => StepOutcome::Exhausted,
        }
    }
}
