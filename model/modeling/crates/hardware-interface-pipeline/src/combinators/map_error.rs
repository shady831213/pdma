use crate::{Stage, StepOutcome, StepResult};
/// Changes application errors without changing the driving call contract.
#[derive(Debug)]
pub struct MapError<S, F> {
    stage: S,
    map: F,
}
impl<S, F> MapError<S, F> {
    /// Wraps existing hardware with an application error conversion.
    pub const fn new(stage: S, map: F) -> Self {
        Self { stage, map }
    }
}
impl<C, S, F, E> Stage<C> for MapError<S, F>
where
    S: Stage<C>,
    F: FnMut(S::Error) -> E,
{
    type Input = S::Input;
    type Output = S::Output;
    type Error = E;
    fn step(
        &mut self,
        context: &mut C,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, E> {
        Ok(match self.stage.step(context, input)? {
            StepOutcome::InProgress => StepOutcome::InProgress,
            StepOutcome::Complete(result) => StepOutcome::Complete(result.map_err(&mut self.map)),
        })
    }
}
