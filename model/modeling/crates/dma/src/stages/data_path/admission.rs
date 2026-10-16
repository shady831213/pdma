use crate::{ChunkInput, CopyProgress, Error, ExecutionContext};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};

#[derive(Debug)]
pub struct Install;

impl<M, SB> Stage<ExecutionContext<'_, M, SB>> for Install {
    type Input = Option<CopyProgress>;
    type Output = Option<ChunkInput>;
    type Error = Error;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Error> {
        let Some(input) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        let result = match input {
            Some(progress) => context.install_progress(progress).map(Some),
            None => Ok(context
                .active
                .filter(|work| *work == context.work())
                .map(ChunkInput::next)),
        };
        Ok(StepOutcome::Complete(result))
    }
}
