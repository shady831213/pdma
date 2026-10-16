use crate::{Fan, FanPolicy, Stage, StepResult};
mod arbitrate;
pub use arbitrate::{Arbitrate, Arbitration};
/// N-to-one specialization of the shared fan lifecycle.
#[derive(Debug)]
pub struct FanIn<S, P, T, I, V, J, O, E>(Fan<S, P, T, I, V, J, O, E>);
impl<S, P, T, I, V, J, O, E> FanIn<S, P, T, I, V, J, O, E> {
    /// Connects multiple input stages to one reusable output stage.
    pub fn new(inputs: Vec<S>, policy: P, output: T) -> Self {
        Self(Fan::new(inputs, policy, vec![output]))
    }
}
impl<C, S, P, T, I, V, J, O, E> Stage<C> for FanIn<S, P, T, I, V, J, O, E>
where
    S: Stage<C, Input = I, Output = V, Error = E>,
    T: Stage<C, Input = J, Output = O, Error = S::Error>,
    P: FanPolicy<S::Output, T::Input, S::Error>,
{
    type Input = Vec<S::Input>;
    type Output = <Fan<S, P, T, I, V, J, O, E> as Stage<C>>::Output;
    type Error = <Fan<S, P, T, I, V, J, O, E> as Stage<C>>::Error;
    fn step(
        &mut self,
        context: &mut C,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        self.0.step(context, input)
    }
}
