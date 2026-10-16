use crate::{Completion, Error, ExecutionContext};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};

/// Merges a normal write completion with a retained stopping command response.
#[derive(Debug)]
pub struct CompletionInput;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for CompletionInput {
    type Input = Option<Completion>;
    type Output = Option<Completion>;
    type Error = Error;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Error> {
        let Some(completion) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
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
