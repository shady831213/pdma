use crate::{BranchError, Stage, StepOutcome, Then};

use super::{
    branches::{Branches, RunBranch},
    flow::{FlowState, Frame, start},
    reduce::Reduce,
};

mod stages;
use stages::Split;

/// Planning result containing application metadata and ordered destination requests.
pub type DeliveryPlan<T, I, E> = Result<(T, Vec<(usize, I)>), BranchError<E>>;

/// Application-owned delivery planning and receipt reduction.
///
/// Splitting prepares destination inputs; reduction consumes the retained
/// metadata and results after all selected destinations complete.
pub trait Distribute<C, S: Stage<C>> {
    /// Owned request to split among selected destinations.
    type Input;
    /// Aggregate returned after all selected destinations complete.
    type Output;
    /// Application metadata retained alongside destination work.
    type State;

    /// Produces an ordered delivery plan without committing destination effects.
    ///
    /// Called once per operation. The application owns splitting, cloning, or
    /// sharing payloads; indices must be distinct and within the destination list.
    fn split(
        &mut self,
        context: &C,
        input: Self::Input,
    ) -> DeliveryPlan<Self::State, S::Input, S::Error>;

    /// Reduces owned receipts in delivery-plan order, including an empty plan.
    fn combine(
        &mut self,
        state: Self::State,
        outputs: Vec<S::Output>,
    ) -> Result<Self::Output, BranchError<S::Error>>;
}

/// Delivers application-defined payloads through retained, ordered destination work.
///
/// Every plan is validated before any destination starts. Each step advances at
/// most one destination; pending work retains prior receipts and later payloads.
/// Failure stops remaining deliveries without reverting committed effects.
#[derive(Debug)]
pub struct FanOut<S, P> {
    destinations: Vec<S>,
    policy: P,
}

/// Owned input, application metadata, and destination progress of one operation.
#[derive(Debug)]
pub struct FanOutState<I, P, D, T, O> {
    pipeline: FlowState<I, P, D, T, O>,
}

impl<S, P> FanOut<S, P> {
    /// Owns the destination population and application delivery policy.
    pub const fn new(destinations: Vec<S>, policy: P) -> Self {
        Self {
            destinations,
            policy,
        }
    }
}

impl<C, S, P> Stage<C> for FanOut<S, P>
where
    S: Stage<C>,
    P: Distribute<C, S>,
{
    type Input = P::Input;
    type Output = P::Output;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = FanOutState<P::Input, P::State, S::Input, S::State, S::Output>;

    fn start(input: Self::Input) -> Self::State {
        let pipeline = Then::new(
            Split,
            Then::new(
                Branches::<P::State>::new(RunBranch::new()),
                Reduce::new(P::combine),
            ),
        );
        FanOutState {
            pipeline: start::<Frame<'_, C, S, P>, _>(&pipeline, input),
        }
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let mut pipeline = Then::new(
            Split,
            Then::new(
                Branches::<P::State>::new(RunBranch::new()),
                Reduce::new(P::combine),
            ),
        );
        pipeline.step(
            &mut Frame {
                context,
                ports: &mut self.destinations,
                policy: &mut self.policy,
            },
            &mut state.pipeline,
        )
    }
}
