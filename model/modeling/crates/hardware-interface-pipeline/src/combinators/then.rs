use crate::{ReadyStage, Stage, StepOutcome};

/// Persistent hardware stages connected by matching output and input ports.
#[derive(Debug)]
pub struct Then<A, B> {
    /// Hardware producing the intermediate value.
    pub upstream: A,
    /// Hardware consuming the intermediate value.
    pub downstream: B,
}

/// Owned state of one request passing through a composed stage.
#[derive(Debug)]
pub enum ThenState<A, B> {
    /// The upstream operation is active.
    Upstream(A),
    /// Upstream finished; its result belongs to downstream operation state.
    Downstream(B),
}

impl<A, B> Then<A, B> {
    /// Connects two hardware objects without creating an operation.
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
    B: Stage<C, Input = A::Output, Error = A::Error, Wait = A::Wait>,
{
    type Input = A::Input;
    type Output = B::Output;
    type Error = A::Error;
    type Wait = A::Wait;
    type State = ThenState<A::State, B::State>;

    fn start(input: Self::Input) -> Self::State {
        ThenState::Upstream(A::start(input))
    }

    fn step(
        &mut self,
        context: &mut C,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        match state {
            ThenState::Upstream(upstream) => match self.upstream.step(context, upstream) {
                StepOutcome::Progressed(state) => {
                    StepOutcome::Progressed(ThenState::Upstream(state))
                }
                StepOutcome::Pending(state, wait) => {
                    StepOutcome::Pending(ThenState::Upstream(state), wait)
                }
                StepOutcome::Complete(Ok(output)) => {
                    StepOutcome::Progressed(ThenState::Downstream(B::start(output)))
                }
                StepOutcome::Complete(Err(error)) => StepOutcome::Complete(Err(error)),
            },
            ThenState::Downstream(downstream) => match self.downstream.step(context, downstream) {
                StepOutcome::Progressed(state) => {
                    StepOutcome::Progressed(ThenState::Downstream(state))
                }
                StepOutcome::Pending(state, wait) => {
                    StepOutcome::Pending(ThenState::Downstream(state), wait)
                }
                StepOutcome::Complete(result) => StepOutcome::Complete(result),
            },
        }
    }
}

impl<C, A, B> ReadyStage<C> for Then<A, B>
where
    A: ReadyStage<C>,
    B: Stage<C, Input = A::Output, Error = A::Error, Wait = A::Wait>,
{
    fn ready(&self, context: &C) -> bool {
        self.upstream.ready(context)
    }
}

/// Static composition for reusable hardware stages.
pub trait PipelineExt<C>: Stage<C> + Sized {
    /// Connects an existing downstream stage with a matching input port.
    fn then<B>(self, downstream: B) -> Then<Self, B>
    where
        B: Stage<C, Input = Self::Output, Error = Self::Error, Wait = Self::Wait>,
    {
        Then::new(self, downstream)
    }
}

impl<C, A: Stage<C>> PipelineExt<C> for A {}
