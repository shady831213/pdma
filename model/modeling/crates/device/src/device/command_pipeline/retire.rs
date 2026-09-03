use hardware_interface_pipeline::{Stage, StepOutcome};

use super::super::{DeviceCore, ModelStepStatus};
use crate::execution::{CommandRoute, ExecutionFault, RetirableExecution, SuccessfulExecution};
use crate::queue::io::NormalizedFault;
use crate::retirement::{RetirementPorts, RetirementStatus};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) struct Terminal {
    status: Option<ModelStepStatus>,
}

#[derive(Debug)]
pub(super) struct Success {
    outcome: Option<SuccessfulExecution>,
}

#[derive(Debug)]
pub(super) struct ExecutionFailure {
    outcome: Option<ExecutionFault>,
}

#[derive(Debug)]
pub(super) struct FrontendFailure {
    fault: Option<NormalizedFault>,
}

#[derive(Debug)]
pub(super) struct InvalidCompletion {
    route: Option<CommandRoute>,
}

#[derive(Debug)]
pub(super) struct PendingCompletion {
    active: bool,
}

#[derive(Debug)]
pub(super) struct PendingFault {
    active: bool,
}

impl Terminal {
    pub(super) const fn new(status: ModelStepStatus) -> Self {
        Self {
            status: Some(status),
        }
    }
}

impl Success {
    pub(super) const fn new(outcome: SuccessfulExecution) -> Self {
        Self {
            outcome: Some(outcome),
        }
    }
}

impl ExecutionFailure {
    pub(super) const fn new(outcome: ExecutionFault) -> Self {
        Self {
            outcome: Some(outcome),
        }
    }
}

impl FrontendFailure {
    pub(super) const fn new(fault: NormalizedFault) -> Self {
        Self { fault: Some(fault) }
    }
}

impl InvalidCompletion {
    pub(super) const fn new(route: CommandRoute) -> Self {
        Self { route: Some(route) }
    }
}

impl PendingCompletion {
    pub(super) const fn new() -> Self {
        Self { active: true }
    }
}

impl PendingFault {
    pub(super) const fn new() -> Self {
        Self { active: true }
    }
}

type RetireOutcome = StepOutcome<ModelStepStatus, ModelError, AsyncStallReason>;

impl Stage<DeviceCore> for Terminal {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, _: &mut DeviceCore) -> RetireOutcome {
        self.status.take().map_or(StepOutcome::Exhausted, |status| {
            StepOutcome::Complete(Ok(status))
        })
    }
}

impl Stage<DeviceCore> for Success {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        let Some(outcome) = self.outcome.take() else {
            return StepOutcome::Exhausted;
        };
        finish(retire(context, RetirableExecution::Success(outcome)))
    }
}

impl Stage<DeviceCore> for ExecutionFailure {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        let Some(outcome) = self.outcome.take() else {
            return StepOutcome::Exhausted;
        };
        finish(retire(context, RetirableExecution::Fault(outcome)))
    }
}

impl Stage<DeviceCore> for FrontendFailure {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        let Some(fault) = self.fault.take() else {
            return StepOutcome::Exhausted;
        };
        finish(cache_frontend_fault(context, fault))
    }
}

impl Stage<DeviceCore> for InvalidCompletion {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        let Some(route) = self.route.take() else {
            return StepOutcome::Exhausted;
        };
        StepOutcome::Complete(context.queues.consume(route.queue).and_then(|()| {
            if context.queues.has_pending(route.queue)? {
                context.scheduler.enqueue(route.queue);
            }
            Err(ModelError::UnknownCompletionRing(route.completion_ring))
        }))
    }
}

impl Stage<DeviceCore> for PendingCompletion {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        if !self.active {
            return StepOutcome::Exhausted;
        }
        match finish(retry(context)) {
            StepOutcome::Pending(wait) => StepOutcome::Pending(wait),
            terminal => {
                self.active = false;
                terminal
            }
        }
    }
}

impl Stage<DeviceCore> for PendingFault {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        if !self.active {
            return StepOutcome::Exhausted;
        }
        match finish(retry(context)) {
            StepOutcome::Pending(wait) => StepOutcome::Pending(wait),
            terminal => {
                self.active = false;
                terminal
            }
        }
    }
}

const fn finish(status: Result<RetirementStatus, ModelError>) -> RetireOutcome {
    match status {
        Ok(RetirementStatus::Progressed) => StepOutcome::Complete(Ok(ModelStepStatus::Progressed)),
        Ok(RetirementStatus::FaultCached) => {
            StepOutcome::Complete(Ok(ModelStepStatus::FaultCached))
        }
        Ok(RetirementStatus::Stalled(reason)) => StepOutcome::Pending(reason),
        Err(error) => StepOutcome::Complete(Err(error)),
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
    fault: NormalizedFault,
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
