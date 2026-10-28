use hw_model_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_dma::{ChunkInput, ExecutionContext, Flow};
use std::{cell::Cell, rc::Rc};

pub(super) struct Stalled(
    pub(super) Option<Flow<ChunkInput>>,
    pub(super) Rc<Cell<usize>>,
);
impl<M> Stage<ExecutionContext<'_, M>> for Stalled {
    type Input = pdma_model_dma::Flow<ChunkInput>;
    type Output = Flow<pdma_model_dma::Completion>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.0.is_none() {
            self.0 = input.take();
        }
        let Some(flow) = self.0.as_ref() else {
            return Ok(StepOutcome::InProgress);
        };
        self.1.set(self.1.get().max(1));
        Ok(match flow.lifecycle(context) {
            Ok(()) => StepOutcome::InProgress,
            Err(error) => {
                self.1.set(self.1.get() + 1);
                if self.1.get() == 2 {
                    return Ok(StepOutcome::InProgress);
                }
                StepOutcome::Complete(Ok(self
                    .0
                    .take()
                    .ok_or(hw_model_pipeline::RunError::Exhausted)?
                    .stop(error)))
            }
        })
    }
}

pub(super) struct PendingTail;
impl<M> Stage<ExecutionContext<'_, M>> for PendingTail {
    type Input = Flow<pdma_model_dma::Completion>;
    type Output = Flow<pdma_model_dma::Completion>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        _: &mut ExecutionContext<'_, M>,
        _: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        Ok(StepOutcome::InProgress)
    }
}
