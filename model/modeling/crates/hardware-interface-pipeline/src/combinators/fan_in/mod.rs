use crate::{Fan, FanPolicy, Stage, StepResult};
mod arbitrate;
pub use arbitrate::{Arbitrate, Arbitration};
/// N-to-one specialization of the shared fan lifecycle.
#[derive(Debug)]
pub struct FanIn<S, P, T>(Fan<S, P, T>);
impl<S, P, T> FanIn<S, P, T> {
    /// Connects multiple input stages to one reusable output stage.
    pub fn new(inputs: Vec<S>, policy: P, output: T) -> Self {
        Self(Fan::new(inputs, policy, vec![output]))
    }
}
impl<C, S, P, T> Stage<C> for FanIn<S, P, T>
where
    S: Stage<C>,
    T: Stage<C, Error = S::Error>,
    P: FanPolicy<S::Output, T::Input, S::Error>,
{
    type Input = Vec<S::Input>;
    type Output = <Fan<S, P, T> as Stage<C>>::Output;
    type Error = <Fan<S, P, T> as Stage<C>>::Error;
    type State = <Fan<S, P, T> as Stage<C>>::State;
    fn start(input: Self::Input) -> Self::State {
        Fan::<S, P, T>::start(input)
    }
    fn step(
        &mut self,
        context: &mut C,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        self.0.step(context, state)
    }
}
