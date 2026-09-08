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
pub(super) struct PipelineSlot {
    state: Option<PipelineState>,
}

impl PipelineSlot {
    pub(super) const fn new() -> Self {
        Self { state: None }
    }

    pub(super) fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        stages: &mut Then<&mut IssueUnit, Then<&mut ExecutionUnit, &mut RetirementUnit>>,
    ) -> CommandStepOutcome {
        let retirement_pending = stages.downstream.downstream.pending().is_some();
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
        match state {
            PipelineState::Command(state) => stages.step(context, state),
            PipelineState::Retirement(state) => stages.downstream.downstream.step(context, state),
        }
    }

    pub(super) fn clear(&mut self) {
        self.state = None;
    }

    #[cfg(test)]
    pub(super) const fn is_empty(&self) -> bool {
        self.state.is_none()
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
