use crate::{BranchError, ReadyStage, Stage, StepOutcome};

use super::{Combine, CombineOutcome, FanIn};

/// A selection continuation or completed metadata and selected source index.
pub type ArbitrationOutcome<T, E, W> = StepOutcome<T, (T, usize), E, W>;

/// Application policy selecting one ready source per operation.
pub trait Arbitration<C, S: ReadyStage<C>> {
    /// Selection state retained independently of the policy hardware.
    type State;
    /// Starts a fresh selection without accessing hardware.
    fn start() -> Self::State;
    /// Selects a source without consuming its input.
    fn select_index(
        &mut self,
        context: &C,
        sources: &[S],
        state: Self::State,
    ) -> ArbitrationOutcome<Self::State, S::Error, S::Wait>;
}

/// Adapts an application arbitration policy to singleton fan-in.
#[derive(Debug)]
pub struct Select<A>(A);

impl<C, S, A> Combine<C, S> for Select<A>
where
    S: ReadyStage<C>,
    A: Arbitration<C, S>,
{
    type Output = S::Output;
    type State = A::State;
    fn start() -> Self::State {
        A::start()
    }

    fn plan(
        &mut self,
        context: &C,
        sources: &[S],
        state: Self::State,
    ) -> CombineOutcome<Self::State, S::Error, S::Wait> {
        if sources.is_empty() {
            return StepOutcome::Complete(Err(BranchError::EmptySources));
        }
        match self.0.select_index(context, sources, state) {
            StepOutcome::Progressed(state) => StepOutcome::Progressed(state),
            StepOutcome::Pending(state, wait) => StepOutcome::Pending(state, wait),
            StepOutcome::Complete(result) => StepOutcome::Complete(
                result
                    .map(|(state, index)| (state, vec![index]))
                    .map_err(BranchError::Application),
            ),
        }
    }

    fn combine(
        &mut self,
        _: Self::State,
        mut outputs: Vec<S::Output>,
    ) -> Result<S::Output, BranchError<S::Error>> {
        let actual = outputs.len();
        if actual != 1 {
            return Err(BranchError::OutputCount {
                expected: 1,
                actual,
            });
        }
        outputs.pop().ok_or(BranchError::OutputCount {
            expected: 1,
            actual,
        })
    }
}

/// Singleton specialization of the shared fan-in lifecycle.
#[derive(Debug)]
pub struct Arbitrate<S, A>(FanIn<S, Select<A>>);

impl<S, A> Arbitrate<S, A> {
    /// Owns the source population and application selection policy.
    pub const fn new(sources: Vec<S>, policy: A) -> Self {
        Self(FanIn::new(sources, Select(policy)))
    }
}

impl<C, S, A> Stage<C> for Arbitrate<S, A>
where
    S: ReadyStage<C>,
    A: Arbitration<C, S>,
{
    type Input = ();
    type Output = S::Output;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = <FanIn<S, Select<A>> as Stage<C>>::State;
    fn start((): ()) -> Self::State {
        FanIn::<S, Select<A>>::start(())
    }
    fn step(
        &mut self,
        context: &mut C,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        if self
            .0
            .unstarted_source(&state)
            .is_some_and(|source| !source.ready(context))
        {
            return StepOutcome::Progressed(FanIn::<S, Select<A>>::start(()));
        }
        self.0.step(context, state)
    }
}

impl<C, S, A> ReadyStage<C> for Arbitrate<S, A>
where
    S: ReadyStage<C>,
    A: Arbitration<C, S>,
{
    fn ready(&self, context: &C) -> bool {
        self.0.fan.ports.iter().any(|source| source.ready(context))
    }
}
