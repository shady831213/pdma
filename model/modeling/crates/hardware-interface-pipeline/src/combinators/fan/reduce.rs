use std::marker::PhantomData;

use crate::{BranchError, Stage, StepOutcome};

use super::Frame;

pub(in crate::combinators) struct Reduce<M, F> {
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
    type State = Self::Input;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, C, S, P>,
        (metadata, outputs): Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        StepOutcome::Complete((self.reduce)(frame.policy, metadata, outputs))
    }
}
