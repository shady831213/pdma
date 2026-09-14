use crate::{BranchError, Stage, StepOutcome};

use super::fan::{BranchPlan, Fan, FanState, Frame, compose, start};

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
    fan: Fan<S, P>,
}

/// Owned input, application metadata, and destination progress of one operation.
#[derive(Debug)]
pub struct FanOutState<I, P, D, T, O> {
    pipeline: FanState<I, P, D, T, O>,
}

impl<S, P> FanOut<S, P> {
    /// Owns the destination population and application delivery policy.
    pub const fn new(destinations: Vec<S>, policy: P) -> Self {
        Self {
            fan: Fan::new(destinations, policy),
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
        let pipeline = compose(Split, P::combine);
        FanOutState {
            pipeline: start::<Frame<'_, C, S, P>, _>(&pipeline, input),
        }
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.fan
            .step(context, &mut state.pipeline, compose(Split, P::combine))
    }
}

struct Split;

impl<C, S: Stage<C>, P: Distribute<C, S>> Stage<Frame<'_, C, S, P>> for Split {
    type Input = P::Input;
    type Output = BranchPlan<P::State, S::Input, S::Output>;
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = Option<P::Input>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        frame: &mut Frame<'_, C, S, P>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let Some(input) = state.take() else {
            return StepOutcome::Exhausted;
        };
        StepOutcome::Complete(
            frame
                .policy
                .split(frame.context, input)
                .and_then(|(metadata, plan)| BranchPlan::new(metadata, plan, frame.ports.len())),
        )
    }
}
