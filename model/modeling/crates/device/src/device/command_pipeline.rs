use hardware_interface_pipeline::{Stage, StepOutcome, Then, ThenState};

use super::DeviceCore;
use super::context::HardwareContext;
use super::issue::IssueUnit;
use crate::device::execution::ExecutionUnit;
use crate::device::retirement::RetirementUnit;
use crate::device::retirement::stage::RetirementInput;
use crate::{AsyncStallReason, ModelError};

type CommandStages = Then<IssueUnit, Then<ExecutionUnit, RetirementUnit>>;
type CommandState = <CommandStages as Stage<HardwareContext<'static>>>::State;

#[derive(Debug)]
pub(super) struct PipelineSlot {
    state: Option<CommandState>,
}

impl PipelineSlot {
    pub(super) const fn new() -> Self {
        Self { state: None }
    }

    pub(super) fn request(&mut self, retirement_pending: bool) -> &mut CommandState {
        self.state.get_or_insert_with(|| {
            if retirement_pending {
                ThenState::Downstream(ThenState::Downstream(Some(RetirementInput::Retry)))
            } else {
                <CommandStages as Stage<HardwareContext<'_>>>::start(())
            }
        })
    }

    pub(super) fn clear(&mut self) {
        self.state = None;
    }

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
