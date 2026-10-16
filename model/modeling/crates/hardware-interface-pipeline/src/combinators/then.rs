use crate::{Stage, StepOutcome, StepResult};

/// Connects two stages and owns their intermediate value and progress.
#[derive(Debug)]
pub struct Then<A, B, V> {
    /// Produces the intermediate value.
    pub upstream: A,
    /// Consumes the intermediate value.
    pub downstream: B,
    edge: Option<V>,
    phase: Phase,
}

#[derive(Debug)]
enum Phase {
    Idle,
    Upstream,
    Downstream,
}

impl<A, B, V> Then<A, B, V> {
    /// Connects reusable hardware without starting work.
    pub const fn new(upstream: A, downstream: B) -> Self {
        Self {
            upstream,
            downstream,
            edge: None,
            phase: Phase::Idle,
        }
    }
}

impl<C, A, B, V> Stage<C> for Then<A, B, V>
where
    A: Stage<C, Output = V>,
    B: Stage<C, Input = V, Error = A::Error>,
{
    type Input = A::Input;
    type Output = B::Output;
    type Error = A::Error;

    fn step(
        &mut self,
        context: &mut C,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let outcome = match self.phase {
            Phase::Idle if input.is_none() => return Ok(StepOutcome::InProgress),
            Phase::Idle | Phase::Upstream => {
                let mut no_offer = None;
                let offered = match self.phase {
                    Phase::Idle => input,
                    Phase::Upstream | Phase::Downstream => &mut no_offer,
                };
                match self.upstream.step(context, offered) {
                    Ok(StepOutcome::InProgress) => {
                        if offered.is_none() {
                            self.phase = Phase::Upstream;
                        }
                        return Ok(StepOutcome::InProgress);
                    }
                    Ok(StepOutcome::Complete(Ok(output))) => {
                        self.edge = Some(output);
                        self.phase = Phase::Downstream;
                        return Ok(StepOutcome::InProgress);
                    }
                    Ok(StepOutcome::Complete(Err(error))) => Ok(StepOutcome::Complete(Err(error))),
                    Err(error) => {
                        if offered.is_none() {
                            self.phase = Phase::Upstream;
                        }
                        Err(error)
                    }
                }
            }
            Phase::Downstream => self.downstream.step(context, &mut self.edge),
        };
        if matches!(outcome, Ok(StepOutcome::Complete(_))) {
            self.edge = None;
            self.phase = Phase::Idle;
        }
        outcome
    }
}

/// Static composition for reusable stages.
pub trait PipelineExt<C>: Stage<C> + Sized {
    /// Connects a downstream stage with matching value and error types.
    fn then<B>(self, downstream: B) -> Then<Self, B, Self::Output>
    where
        B: Stage<C, Input = Self::Output, Error = Self::Error>,
    {
        Then::new(self, downstream)
    }
}
impl<C, S: Stage<C>> PipelineExt<C> for S {}
