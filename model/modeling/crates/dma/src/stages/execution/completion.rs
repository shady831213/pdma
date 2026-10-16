use crate::{Completion, ExecutionContext, Flow};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};

/// Merges a normal write completion with a retained stopping command response.
#[derive(Debug)]
pub struct CompletionInput;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for CompletionInput {
    type Input = Flow<Completion>;
    type Output = Flow<Completion>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(completion) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        let mut state = Some(completion);
        Flow::advance(&mut state, context, |completion, context| {
            let work = context.work();
            let result = context.channel(work).map(|channel| {
                completion.take().or_else(|| {
                    channel
                        .work
                        .as_mut()
                        .and_then(|accepted| accepted.completion.take())
                })
            });
            Ok(StepOutcome::Complete(result))
        })
    }
}
