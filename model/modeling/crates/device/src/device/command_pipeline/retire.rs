use hardware_interface_pipeline::{Stage, StepOutcome};

use super::super::{DeviceCore, ModelStepStatus};
use crate::execution::{CommandRoute, ExecutionFault, RetirableExecution, SuccessfulExecution};
use crate::queue::io::NormalizedFault;
use crate::retirement::{RetirementPorts, RetirementStatus};
use crate::{AsyncStallReason, ModelError};

#[derive(Debug)]
pub(super) struct Success {
    outcome: Option<SuccessfulExecution>,
    retry: PendingCompletion,
    active: bool,
}

#[derive(Debug)]
pub(super) struct ExecutionFailure {
    outcome: Option<ExecutionFault>,
    retry: PendingFault,
    active: bool,
}

#[derive(Debug)]
pub(super) struct FrontendFailure {
    fault: Option<NormalizedFault>,
    retry: PendingFault,
    active: bool,
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

impl Success {
    pub(super) const fn new(outcome: SuccessfulExecution) -> Self {
        Self {
            outcome: Some(outcome),
            retry: PendingCompletion::new(),
            active: true,
        }
    }
}

impl ExecutionFailure {
    pub(super) const fn new(outcome: ExecutionFault) -> Self {
        Self {
            outcome: Some(outcome),
            retry: PendingFault::new(),
            active: true,
        }
    }
}

impl FrontendFailure {
    pub(super) const fn new(fault: NormalizedFault) -> Self {
        Self {
            fault: Some(fault),
            retry: PendingFault::new(),
            active: true,
        }
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

impl Stage<DeviceCore> for Success {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        if !self.active {
            return StepOutcome::Exhausted;
        }
        let outcome = match self.outcome.take() {
            Some(outcome) => finish(retire(context, RetirableExecution::Success(outcome))),
            None => self.retry.step(context),
        };
        retain_pending(&mut self.active, outcome)
    }
}

impl Stage<DeviceCore> for ExecutionFailure {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        if !self.active {
            return StepOutcome::Exhausted;
        }
        let outcome = match self.outcome.take() {
            Some(outcome) => finish(retire(context, RetirableExecution::Fault(outcome))),
            None => self.retry.step(context),
        };
        retain_pending(&mut self.active, outcome)
    }
}

impl Stage<DeviceCore> for FrontendFailure {
    type Error = ModelError;
    type Output = ModelStepStatus;
    type Wait = AsyncStallReason;

    fn step(&mut self, context: &mut DeviceCore) -> RetireOutcome {
        if !self.active {
            return StepOutcome::Exhausted;
        }
        let outcome = match self.fault.take() {
            Some(fault) => finish(cache_frontend_fault(context, fault)),
            None => self.retry.step(context),
        };
        retain_pending(&mut self.active, outcome)
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

const fn retain_pending(active: &mut bool, outcome: RetireOutcome) -> RetireOutcome {
    if !matches!(outcome, StepOutcome::Pending(_)) {
        *active = false;
    }
    outcome
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
