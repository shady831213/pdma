use crate::{Completion, Error, ExecutionContext};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};

/// Merges a normal write completion with a retained stopping command response.
#[derive(Debug)]
pub struct CompletionInput;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for CompletionInput {
    type Input = Option<Completion>;
    type Output = Option<Completion>;
    type Error = Error;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let completion = state.take().ok_or(RunError::Exhausted)?;
        let work = context.work();
        let result = context.channel(work).map(|channel| {
            completion.or_else(|| {
                channel
                    .work
                    .as_mut()
                    .and_then(|accepted| accepted.completion.take())
            })
        });
        Ok(StepOutcome::Complete(result))
    }
}
