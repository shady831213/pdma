use crate::{BranchError, Stage, StepOutcome, Then, ThenState};

use super::{
    branches::{Branches, RunBranch, RunBranchState},
    flow::{FlowState, Frame, start},
    reduce::Reduce,
};

mod stages;
use stages::Plan;

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
    pub(super) sources: Vec<S>,
    policy: P,
}

/// Retained planning, accepted child work, and partial aggregate.
#[derive(Debug)]
pub struct FanInState<P, T, O> {
    pipeline: FlowState<P, P, (), T, O>,
}

impl<S, P> FanIn<S, P> {
    /// Owns the sources and application policy for the hardware lifetime.
    pub const fn new(sources: Vec<S>, policy: P) -> Self {
        Self { sources, policy }
    }

    pub(super) fn unstarted_source<M, T, O>(&self, state: &FanInState<M, T, O>) -> Option<&S> {
        match &state.pipeline {
            ThenState::Downstream(next) => match next {
                ThenState::Upstream(branches) => branches
                    .as_ref()
                    .and_then(RunBranchState::unstarted_index)
                    .and_then(|index| self.sources.get(index)),
                ThenState::Downstream(_) | ThenState::Done => None,
            },
            ThenState::Upstream(_) | ThenState::Done => None,
        }
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
        let pipeline = Then::new(
            Plan,
            Then::new(
                Branches::<P::State>::new(RunBranch::new()),
                Reduce::new(P::combine),
            ),
        );
        FanInState {
            pipeline: start::<Frame<'_, C, S, P>, _>(&pipeline, ()),
        }
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let mut pipeline = Then::new(
            Plan,
            Then::new(
                Branches::<P::State>::new(RunBranch::new()),
                Reduce::new(P::combine),
            ),
        );
        pipeline.step(
            &mut Frame {
                context,
                ports: &mut self.sources,
                policy: &mut self.policy,
            },
            &mut state.pipeline,
        )
    }
}
