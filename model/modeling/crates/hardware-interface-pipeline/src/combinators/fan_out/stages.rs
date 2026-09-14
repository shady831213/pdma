use crate::{BranchError, Distribute, Stage, StepOutcome};

use super::super::{branches::BranchPlan, flow::Frame};

pub(super) struct Split;

impl<C, S: Stage<C>, P: Distribute<C, S>> Stage<Frame<'_, C, S, P>> for Split {
    type Input = P::Input;
    type Output = BranchPlan<P::State, S::Input, S::Output>;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = Option<P::Input>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, C, S, P>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(input) = state.take() else {
            return StepOutcome::Exhausted;
        };
        StepOutcome::Complete(
            frame
                .policy
                .split(frame.context, input)
                .and_then(|(metadata, plan)| BranchPlan::new(metadata, plan, frame.ports.len())),
        )
    }
}
