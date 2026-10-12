use crate::{BranchError, Stage, StepOutcome};

use super::{DeliveryPlan, Distribute, FanOut, FanOutState};

/// Application policy routing one owned request to one destination.
pub trait Route<C, S: Stage<C>> {
    /// Request to transform into a destination input.
    type Input;

    /// Chooses a destination and prepares its input once, before any delivery.
    ///
    /// # Errors
    /// Returns the application's routing failure without starting a destination.
    fn route(&mut self, context: &C, input: Self::Input) -> Result<(usize, S::Input), S::Error>;
}

/// Singleton fan-out returning unit after the selected destination succeeds.
///
/// Routing, index validation, suspension and terminality use the existing
/// fan-out lifecycle. Applications need no receipt reducer or operation metadata.
#[derive(Debug)]
pub struct Dispatch<S, R>(FanOut<S, Single<R>>);

impl<S, R> Dispatch<S, R> {
    /// Owns the destination population and application routing policy.
    pub const fn new(destinations: Vec<S>, route: R) -> Self {
        Self(FanOut::new(destinations, Single(route)))
    }
}

impl<C, S: Stage<C>, R: Route<C, S>> Stage<C> for Dispatch<S, R> {
    type Input = R::Input;
    type Output = ();
    type Error = BranchError<S::Error>;
    type Wait = S::Wait;
    type State = FanOutState<R::Input, (), S::Input, S::State, S::Output>;

    fn start(input: Self::Input) -> Self::State {
        <FanOut<S, Single<R>> as Stage<C>>::start(input)
    }

    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepOutcome<(), Self::Error, Self::Wait> {
        self.0.step(context, state)
    }
}

#[derive(Debug)]
struct Single<R>(R);

impl<C, S: Stage<C>, R: Route<C, S>> Distribute<C, S> for Single<R> {
    type Input = R::Input;
    type Output = ();
    type State = ();

    fn split(&mut self, context: &C, input: Self::Input) -> DeliveryPlan<(), S::Input, S::Error> {
        self.0
            .route(context, input)
            .map(|delivery| ((), vec![delivery]))
            .map_err(BranchError::Application)
    }

    fn combine(&mut self, (): (), _: Vec<S::Output>) -> Result<(), BranchError<S::Error>> {
        Ok(())
    }
}
