use hardware_interface_pipeline::{Stage, StepOutcome, Then};

use super::DeviceCore;
use super::ModelStepStatus;
use super::context::HardwareContext;
use super::drive::CommandStepOutcome;
use super::issue::IssueUnit;
use crate::device::execution::ExecutionUnit;
use crate::device::retirement::RetirementUnit;
use crate::device::retirement::stage::RetirementState;
use crate::{AsyncStallReason, ModelError};

type CommandStages = Then<IssueUnit, Then<ExecutionUnit, RetirementUnit>>;
type CommandState = <CommandStages as Stage<HardwareContext<'static>>>::State;

#[derive(Debug)]
enum PipelineState {
    Command(CommandState),
    Retirement(RetirementState),
}

#[derive(Debug)]
pub(super) struct CommandPipeline {
    state: Option<PipelineState>,
    pub(super) issue: IssueUnit,
    pub(super) execution: ExecutionUnit,
    pub(super) retirement: RetirementUnit,
}

impl CommandPipeline {
    pub(super) fn new(resources: pdma_model_contract::semantic::ResourceConfig) -> Self {
        Self {
            state: None,
            issue: IssueUnit::default(),
            execution: ExecutionUnit::new(resources),
            retirement: RetirementUnit::default(),
        }
    }

    pub(super) const fn clear(&mut self) {
        self.state = None;
    }

    #[cfg(test)]
    pub(super) const fn is_empty(&self) -> bool {
        self.state.is_none()
    }
}

impl Stage<HardwareContext<'_>> for CommandPipeline {
    type Input = ();
    type Output = ModelStepStatus;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = ();

    fn start((): Self::Input) -> Self::State {}

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        (): &mut Self::State,
    ) -> CommandStepOutcome {
        let retirement_pending = self.retirement.pending().is_some();
        if self.state.is_none()
            && !retirement_pending
            && context.runnable.current().is_none()
            && !context.queues.has_any_pending()
        {
            context.trace.idle();
            return StepOutcome::Complete(Ok(ModelStepStatus::Idle));
        }
        let state = self.state.get_or_insert_with(|| {
            if retirement_pending {
                PipelineState::Retirement(RetirementState::Retry)
            } else {
                PipelineState::Command(<CommandStages as Stage<HardwareContext<'_>>>::start(()))
            }
        });
        let outcome = match state {
            PipelineState::Command(state) => Then::new(
                &mut self.issue,
                Then::new(&mut self.execution, &mut self.retirement),
            )
            .step(context, state),
            PipelineState::Retirement(state) => self.retirement.step(context, state),
        };
        if matches!(outcome, StepOutcome::Complete(_) | StepOutcome::Exhausted) {
            self.clear();
        }
        outcome
    }
}

impl Stage<()> for DeviceCore {
    type Input = ();
    type Output = crate::HardwareStepStatus;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = ();

    fn start((): Self::Input) -> Self::State {}

    fn step(
        &mut self,
        (): &mut (),
        (): &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.step_pipeline()
    }
}

#[cfg(test)]
mod tests;
