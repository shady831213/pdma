use std::marker::PhantomData;

use crate::{BranchError, Stage, StepOutcome};

use super::flow::Frame;

pub(super) struct Reduce<M, F> {
    reduce: F,
    metadata: PhantomData<fn() -> M>,
}

impl<M, F> Reduce<M, F> {
    pub(super) const fn new(reduce: F) -> Self {
        Self {
            reduce,
            metadata: PhantomData,
        }
    }
}

impl<C, S, P, M, O, F> Stage<Frame<'_, C, S, P>> for Reduce<M, F>
where
    S: Stage<C>,
    F: FnMut(&mut P, M, Vec<S::Output>) -> Result<O, BranchError<S::Error>>,
{
    type Input = (M, Vec<S::Output>);
    type Output = O;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = Option<Self::Input>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, C, S, P>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some((metadata, outputs)) = state.take() else {
            return StepOutcome::Exhausted;
        };
        StepOutcome::Complete((self.reduce)(frame.policy, metadata, outputs))
    }
}
