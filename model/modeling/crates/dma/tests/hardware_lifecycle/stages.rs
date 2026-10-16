use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_dma::{ChunkInput, Error, ExecutionContext};

pub(super) struct Stalled(pub(super) bool);
impl<M> Stage<ExecutionContext<'_, M>> for Stalled {
    type Input = Option<ChunkInput>;
    type Output = ();
    type Error = Error;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<(), Error> {
        if !self.0 {
            if input.take().is_none() {
                return Ok(StepOutcome::InProgress);
            }
            self.0 = true;
        }
        Ok(match context.lifecycle() {
            Ok(()) => StepOutcome::InProgress,
            Err(error) => {
                self.0 = false;
                StepOutcome::Complete(Err(error))
            }
        })
    }
}

pub(super) struct PendingTail;
impl<M> Stage<ExecutionContext<'_, M>> for PendingTail {
    type Input = ();
    type Output = ();
    type Error = Error;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, M>,
        _: &mut Option<()>,
    ) -> StepResult<(), Error> {
        Ok(StepOutcome::InProgress)
    }
}
