use hardware_interface_pipeline::{Stage, StepOutcome, Then, ThenState};

use super::context::HardwareContext;
use crate::{AsyncStallReason, ModelError};

mod arbitrate;
mod fetch;
#[cfg(test)]
mod tests;

use super::flow::FetchedWork;
use arbitrate::{Arbitration, SelectedQueue};
use fetch::Fetch;

#[derive(Debug)]
pub(super) struct IssueUnit {
    stages: Then<Arbitration, Fetch>,
}

impl Stage<HardwareContext<'_>> for IssueUnit {
    type Input = ();
    type Output = FetchedWork;
    type Error = ModelError;
    type Wait = AsyncStallReason;
    type State = ThenState<bool, Option<SelectedQueue>>;

    fn start((): Self::Input) -> Self::State {
        <Then<Arbitration, Fetch> as Stage<HardwareContext<'_>>>::start(())
    }

    fn step(
        &mut self,
        context: &mut HardwareContext<'_>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        match self.stages.step(context, state) {
            StepOutcome::Progressed => self.stages.step(context, state),
            outcome => outcome,
        }
    }
}

impl Default for IssueUnit {
    fn default() -> Self {
        Self {
            stages: Then::new(Arbitration, Fetch),
        }
    }
}
