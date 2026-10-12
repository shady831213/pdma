use crate::{Fan, FanPolicy, Stage, StepResult};
mod dispatch;
pub use dispatch::{Dispatch, Route};
/// One-to-M specialization of the shared fan lifecycle.
#[derive(Debug)]
pub struct FanOut<S, P, T>(Fan<S, P, T>);
impl<S, P, T> FanOut<S, P, T> {
    /// Connects one input stage to multiple output stages.
    pub fn new(input: S, policy: P, outputs: Vec<T>) -> Self {
        Self(Fan::new(vec![input], policy, outputs))
    }
}
impl<C, S, P, T> Stage<C> for FanOut<S, P, T>
where
    S: Stage<C>,
    T: Stage<C, Error = S::Error>,
    P: FanPolicy<S::Output, T::Input, S::Error>,
{
    type Input = S::Input;
    type Output = <Fan<S, P, T> as Stage<C>>::Output;
    type Error = <Fan<S, P, T> as Stage<C>>::Error;
    type State = <Fan<S, P, T> as Stage<C>>::State;
    fn start(input: Self::Input) -> Self::State {
        Fan::<S, P, T>::start(vec![input])
    }
    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        self.0.step(context, state)
    }
}
