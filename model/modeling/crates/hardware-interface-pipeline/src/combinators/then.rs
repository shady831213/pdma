use crate::{RunError, Stage, StepOutcome, StepResult};

/// Connects the output of one stage to the input of another.
#[derive(Debug)]
pub struct Then<A, B> {
    /// Produces the intermediate value.
    pub upstream: A,
    /// Consumes the intermediate value.
    pub downstream: B,
}
/// Local invocation position in a sequential composition.
#[derive(Debug)]
pub enum ThenState<A, B> {
    /// Upstream work remains.
    Upstream(A),
    /// The transferred value belongs to downstream.
    Downstream(B),
    /// The invocation has terminated.
    Done,
}
impl<A, B> Then<A, B> {
    /// Connects reusable hardware without starting work.
    pub const fn new(upstream: A, downstream: B) -> Self {
        Self {
            upstream,
            downstream,
        }
    }
}
impl<C, A, B> Stage<C> for Then<A, B>
where
    A: Stage<C>,
    B: Stage<C, Input = A::Output, Error = A::Error>,
{
    type Input = A::Input;
    type Output = B::Output;
    type Error = A::Error;
    type State = ThenState<A::State, B::State>;
    fn start(input: Self::Input) -> Self::State {
        ThenState::Upstream(A::start(input))
    }
    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        match std::mem::replace(state, ThenState::Done) {
            ThenState::Upstream(mut upstream) => {
                match self.upstream.step(context, &mut upstream)? {
                    StepOutcome::InProgress => {
                        *state = ThenState::Upstream(upstream);
                        Ok(StepOutcome::InProgress)
                    }
                    StepOutcome::Complete(Ok(output)) => {
                        *state = ThenState::Downstream(B::start(output));
                        Ok(StepOutcome::InProgress)
                    }
                    StepOutcome::Complete(Err(error)) => Ok(StepOutcome::Complete(Err(error))),
                }
            }
            ThenState::Downstream(mut downstream) => {
                let outcome = self.downstream.step(context, &mut downstream)?;
                if matches!(outcome, StepOutcome::InProgress) {
                    *state = ThenState::Downstream(downstream);
                }
                Ok(outcome)
            }
            ThenState::Done => Err(RunError::Exhausted),
        }
    }
}
/// Static composition for reusable stages.
pub trait PipelineExt<C>: Stage<C> + Sized {
    /// Connects a downstream stage with matching value and error types.
    fn then<B>(self, downstream: B) -> Then<Self, B>
    where
        B: Stage<C, Input = Self::Output, Error = Self::Error>,
    {
        Then::new(self, downstream)
    }
}
impl<C, S: Stage<C>> PipelineExt<C> for S {}
