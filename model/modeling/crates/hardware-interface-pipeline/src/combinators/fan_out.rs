use crate::{BranchError, Stage, StepOutcome};

use super::branches::Branches;

/// Planning result containing application metadata and ordered destination requests.
pub type DeliveryPlan<T, I, E> = Result<(T, Vec<(usize, I)>), BranchError<E>>;

/// Application-owned delivery planning and receipt reduction.
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
    phase: Phase<I, P, D, T, O>,
}

#[derive(Debug)]
enum Phase<I, P, D, T, O> {
    Planning(I),
    Active(P, Branches<D, T, O>),
    Done,
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
        FanOutState {
            phase: Phase::Planning(input),
        }
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        let (policy, mut branches) = match std::mem::replace(&mut state.phase, Phase::Done) {
            Phase::Planning(input) => {
                let (policy, plan) = match self.policy.split(context, input) {
                    Ok(plan) => plan,
                    Err(error) => return StepOutcome::Complete(Err(error)),
                };
                match Branches::new(plan, self.destinations.len()) {
                    Ok(branches) => (policy, branches),
                    Err(error) => return StepOutcome::Complete(Err(error)),
                }
            }
            Phase::Active(policy, branches) => (policy, branches),
            Phase::Done => return StepOutcome::Exhausted,
        };
        match branches.step(&mut self.destinations, context) {
            StepOutcome::Progressed => {
                state.phase = Phase::Active(policy, branches);
                StepOutcome::Progressed
            }
            StepOutcome::Pending(wait) => {
                state.phase = Phase::Active(policy, branches);
                StepOutcome::Pending(wait)
            }
            StepOutcome::Complete(result) => StepOutcome::Complete(
                result.and_then(|outputs| self.policy.combine(policy, outputs)),
            ),
            StepOutcome::Exhausted => StepOutcome::Exhausted,
        }
    }
}
