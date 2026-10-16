use crate::{Fan, FanPolicy, Stage, StepResult};
mod dispatch;
pub use dispatch::{Dispatch, Route};
/// One-to-M specialization of the shared fan lifecycle.
#[derive(Debug)]
pub struct FanOut<S, P, T, I, V, J, O, E>(Fan<S, P, T, I, V, J, O, E>);
impl<S, P, T, I, V, J, O, E> FanOut<S, P, T, I, V, J, O, E> {
    /// Connects one input stage to multiple output stages.
    pub fn new(input: S, policy: P, outputs: Vec<T>) -> Self {
        Self(Fan::new(vec![input], policy, outputs))
    }
}
impl<C, S, P, T, I, V, J, O, E> Stage<C> for FanOut<S, P, T, I, V, J, O, E>
where
    S: Stage<C, Input = I, Output = V, Error = E>,
    T: Stage<C, Input = J, Output = O, Error = S::Error>,
    P: FanPolicy<S::Output, T::Input, S::Error>,
{
    type Input = S::Input;
    type Output = <Fan<S, P, T, I, V, J, O, E> as Stage<C>>::Output;
    type Error = <Fan<S, P, T, I, V, J, O, E> as Stage<C>>::Error;
    fn step(
        &mut self,
        context: &mut C,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.0.is_active() {
            return self.0.step(context, &mut None);
        }
        let mut offered = input.take().map(|value| vec![value]);
        let result = self.0.step(context, &mut offered);
        *input = offered.and_then(|mut values| values.pop());
        result
    }
}
