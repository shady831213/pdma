use crate::{Stage, StepOutcome};

/// A statically composed pair of stages.
#[derive(Debug)]
pub struct Then<A, F, B> {
    state: State<A, F, B>,
}

#[derive(Debug)]
enum State<A, F, B> {
    Upstream(A, Option<F>),
    Downstream(B),
    Done,
}

impl<A, F, B> Then<A, F, B> {
    const fn new(upstream: A, bind: F) -> Self {
        Self {
            state: State::Upstream(upstream, Some(bind)),
        }
    }
}

impl<C, A, F, B> Stage<C> for Then<A, F, B>
where
    A: Stage<C>,
    B: Stage<C, Error = A::Error, Wait = A::Wait>,
    F: FnOnce(A::Output) -> B,
{
    type Error = A::Error;
    type Output = B::Output;
    type Wait = A::Wait;

    fn step(&mut self, context: &mut C) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        match std::mem::replace(&mut self.state, State::Done) {
            State::Upstream(mut upstream, bind) => match upstream.step(context) {
                StepOutcome::Progressed => {
                    self.state = State::Upstream(upstream, bind);
                    StepOutcome::Progressed
                }
                StepOutcome::Pending(wait) => {
                    self.state = State::Upstream(upstream, bind);
                    StepOutcome::Pending(wait)
                }
                StepOutcome::Complete(Ok(output)) => match bind {
                    Some(bind) => {
                        self.state = State::Downstream(bind(output));
                        StepOutcome::Progressed
                    }
                    None => StepOutcome::Exhausted,
                },
                StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
                StepOutcome::Exhausted => StepOutcome::Exhausted,
            },
            State::Downstream(mut downstream) => match downstream.step(context) {
                StepOutcome::Progressed => {
                    self.state = State::Downstream(downstream);
                    StepOutcome::Progressed
                }
                StepOutcome::Pending(wait) => {
                    self.state = State::Downstream(downstream);
                    StepOutcome::Pending(wait)
                }
                StepOutcome::Complete(result) => StepOutcome::Complete(result),
                StepOutcome::Exhausted => StepOutcome::Exhausted,
            },
            State::Done => StepOutcome::Exhausted,
        }
    }
}

/// Extension methods for statically composing pipeline stages.
pub trait PipelineExt<C>: Stage<C> + Sized {
    /// Constructs a downstream stage from this stage's owned successful output.
    fn then<F, B>(self, bind: F) -> Then<Self, F, B>
    where
        F: FnOnce(Self::Output) -> B,
    {
        Then::new(self, bind)
    }
}

impl<C, A> PipelineExt<C> for A where A: Stage<C> {}
