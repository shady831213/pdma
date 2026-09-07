mod execute;
mod fetch;
mod retire;
mod route;

#[cfg(test)]
mod tests;

use hardware_interface_pipeline::{PipelineExt, Stage, StepOutcome, Then};

use self::fetch::{FetchedRoute, ProbeFetch};
use self::route::{Operation, Retirement};
use super::{DeviceCore, ModelStepStatus};
use crate::{AsyncStallReason, ModelError};

type FetchOperation = Then<ProbeFetch, fn(FetchedRoute) -> Operation, Operation>;
type CommandStages = Then<FetchOperation, fn(route::RetirementRoute) -> Retirement, Retirement>;

#[derive(Debug)]
struct PdmaCommandPipeline {
    stages: CommandStages,
}

impl PdmaCommandPipeline {
    #[cfg(test)]
    fn new() -> Self {
        Self::compose(ProbeFetch::new())
    }

    fn accepted() -> Self {
        Self::compose(ProbeFetch::accepted())
    }

    fn compose(fetch: ProbeFetch) -> Self {
        let operation = Operation::from_fetch as fn(FetchedRoute) -> Operation;
        let retirement = Retirement::from_route as fn(route::RetirementRoute) -> Retirement;
        let fetch_operation = PipelineExt::<DeviceCore>::then(fetch, operation);
        Self {
            stages: PipelineExt::<DeviceCore>::then(fetch_operation, retirement),
        }
    }

    #[cfg(test)]
    fn from_route(route: FetchedRoute) -> Self {
        Self::compose(ProbeFetch::injected(route))
    }
}

impl Stage<DeviceCore> for PdmaCommandPipeline {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(
        &mut self,
        context: &mut DeviceCore,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.stages.step(context)
    }
}

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
    ) -> StepOutcome<ModelStepStatus, ModelError, AsyncStallReason> {
        let mut pipeline = self
            .pipeline
            .take()
            .unwrap_or_else(PdmaCommandPipeline::accepted);
        let outcome = pipeline.step(context);
        if matches!(outcome, StepOutcome::Progressed | StepOutcome::Pending(_)) {
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
