use crate::{BranchError, Combine, FanIn, ReadyStage, Stage, StepOutcome};

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
        state: &mut Self::State,
    ) -> StepOutcome<usize, S::Error, S::Wait>;
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
        state: &mut Self::State,
    ) -> StepOutcome<Vec<usize>, BranchError<S::Error>, S::Wait> {
        match self.0.select_index(context, sources, state) {
            StepOutcome::Progressed => StepOutcome::Progressed,
            StepOutcome::Pending(wait) => StepOutcome::Pending(wait),
            StepOutcome::Complete(result) => StepOutcome::Complete(
                result
                    .map(|index| vec![index])
                    .map_err(BranchError::Application),
            ),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }

    fn combine(
        &mut self,
        _: &mut Self::State,
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
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.0.step(context, state)
    }
}

impl<C, S, A> ReadyStage<C> for Arbitrate<S, A>
where
    S: ReadyStage<C>,
    A: Arbitration<C, S>,
{
    fn ready(&self, context: &C) -> bool {
        self.0.sources.iter().any(|source| source.ready(context))
    }
}
