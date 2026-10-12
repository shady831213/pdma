use crate::{BranchError, Stage, StepOutcome};

use super::fan::{BranchPlan, Fan, FanState, Frame, compose, start};

mod arbitrate;
pub use arbitrate::{Arbitrate, Arbitration, Select};

/// Application-owned input planning and reduction for one fan-in operation.
///
/// Planning observes sources without consuming them; reduction consumes the
/// retained metadata and branch outputs after all selected sources complete.
pub trait Combine<C, S: Stage<C, Input = ()>> {
    /// Aggregate returned after all selected inputs complete.
    type Output;
    /// Planning and reduction state retained across steps.
    type State;
    /// Starts one operation without interacting with its sources.
    fn start() -> Self::State;
    /// Selects an ordered subset of sources without consuming input.
    fn plan(
        &mut self,
        context: &C,
        sources: &[S],
        state: &mut Self::State,
    ) -> StepOutcome<Vec<usize>, BranchError<S::Error>, S::Wait>;
    /// Reduces owned outputs in plan order.
    ///
    /// # Errors
    /// Returns the application's aggregate failure.
    fn combine(
        &mut self,
        state: Self::State,
        outputs: Vec<S::Output>,
    ) -> Result<Self::Output, BranchError<S::Error>>;
}

/// Gathers application-selected inputs and reduces their owned outputs.
#[derive(Debug)]
pub struct FanIn<S, P> {
    pub(super) fan: Fan<S, P>,
}

/// Retained planning, accepted child work, and partial aggregate.
#[derive(Debug)]
pub struct FanInState<P, T, O> {
    pipeline: FanState<P, P, (), T, O>,
}

impl<S, P> FanIn<S, P> {
    /// Owns the sources and application policy for the hardware lifetime.
    pub const fn new(sources: Vec<S>, policy: P) -> Self {
        Self {
            fan: Fan::new(sources, policy),
        }
    }

    pub(super) fn unstarted_source<M, T, O>(&self, state: &FanInState<M, T, O>) -> Option<&S> {
        self.fan.unstarted_port(&state.pipeline)
    }
}

impl<C, S, P> Stage<C> for FanIn<S, P>
where
    S: Stage<C, Input = ()>,
    P: Combine<C, S>,
{
    type Input = ();
    type Output = P::Output;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = FanInState<P::State, S::State, S::Output>;

    fn start((): ()) -> Self::State {
        let pipeline = compose(Plan, P::combine);
        FanInState {
            pipeline: start::<Frame<'_, C, S, P>, _>(&pipeline, ()),
        }
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.fan
            .step(context, &mut state.pipeline, compose(Plan, P::combine))
    }
}

struct Plan;

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
