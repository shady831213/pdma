use crate::{RunError, Stage, StepOutcome, StepResult};
use std::convert::Infallible;

/// Runs a finite update after each upstream step, including stalls.
/// A stalled update retains the upstream outcome until its own completion.
#[derive(Debug)]
pub struct AfterEach<A, B> {
    /// Hardware driving the invocation result.
    pub upstream: A,
    /// Finite maintenance hardware invoked after each upstream step.
    pub update: B,
}
/// Retained upstream work and any unfinished maintenance invocation.
#[derive(Debug)]
pub struct AfterEachState<A, B, O, E> {
    upstream: Option<A>,
    update: Option<(B, StepOutcome<O, E>)>,
}
impl<A, B> AfterEach<A, B> {
    /// Connects reusable hardware without starting work.
    pub const fn new(upstream: A, update: B) -> Self {
        Self { upstream, update }
    }
}
impl<C, A, B> Stage<C> for AfterEach<A, B>
where
    A: Stage<C>,
    B: Stage<C, Input = (), Output = (), Error = Infallible>,
{
    type Input = A::Input;
    type Output = A::Output;
    type Error = A::Error;
    type State = Option<AfterEachState<A::State, B::State, A::Output, A::Error>>;
    fn start(input: Self::Input) -> Self::State {
        Some(AfterEachState {
            upstream: Some(A::start(input)),
            update: None,
        })
    }
    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        let mut invocation = state.take().ok_or(RunError::Exhausted)?;
        let (mut update, outcome) = if let Some(saved) = invocation.update.take() {
            saved
        } else {
            let upstream = invocation.upstream.as_mut().ok_or(RunError::Exhausted)?;
            let outcome = self.upstream.step(context, upstream)?;
            if matches!(outcome, StepOutcome::Complete(_)) {
                invocation.upstream = None;
            }
            (B::start(()), outcome)
        };
        match self.update.step(context, &mut update)? {
            StepOutcome::InProgress => {
                invocation.update = Some((update, outcome));
                *state = Some(invocation);
                Ok(StepOutcome::InProgress)
            }
            StepOutcome::Complete(Ok(())) => {
                if matches!(outcome, StepOutcome::InProgress) {
                    *state = Some(invocation);
                }
                Ok(outcome)
            }
            StepOutcome::Complete(Err(never)) => match never {},
        }
    }
}
