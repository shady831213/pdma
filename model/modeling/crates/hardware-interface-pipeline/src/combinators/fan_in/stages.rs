use crate::{BranchError, Combine, Stage, StepOutcome};

use super::super::{branches::BranchPlan, flow::Frame};

pub(super) struct Plan;

impl<C, S: Stage<C, Input = ()>, P: Combine<C, S>> Stage<Frame<'_, C, S, P>> for Plan {
    type Input = ();
    type Output = BranchPlan<P::State, (), S::Output>;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = Option<P::State>;

    fn start((): ()) -> Self::State {
        Some(P::start())
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, C, S, P>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(mut metadata) = state.take() else {
            return StepOutcome::Exhausted;
        };
        match frame.policy.plan(frame.context, frame.ports, &mut metadata) {
            StepOutcome::Progressed => {
                *state = Some(metadata);
                StepOutcome::Progressed
            }
            StepOutcome::Pending(wait) => {
                *state = Some(metadata);
                StepOutcome::Pending(wait)
            }
            StepOutcome::Complete(result) => StepOutcome::Complete(result.and_then(|plan| {
                BranchPlan::new(
                    metadata,
                    plan.into_iter().map(|index| (index, ())).collect(),
                    frame.ports.len(),
                )
            })),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}
