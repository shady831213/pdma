use crate::{Chunk, ChunkInput, ExecutionContext, Flow};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{BoundMemory, MemoryAccess};
use pdma_model_contract::dma::{ContextId, DmaAccess};

/// Selects the next bounded chunk using the channel-owned traversal cursors.
#[derive(Debug, Default)]
pub struct PrepareChunk {
    state: Option<Flow<ChunkInput>>,
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for PrepareChunk {
    type Input = Flow<ChunkInput>;
    type Output = Flow<Chunk>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.state.is_none() {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.state = Some(input);
        }
        Flow::advance(&mut self.state, context, |state, context| {
            let Some(input) = state.as_mut() else {
                return Ok(StepOutcome::Complete(Ok(None)));
            };
            let work = input.work;
            match context.ready(work) {
                Ok(true) => {}
                Ok(false) => return Ok(StepOutcome::InProgress),
                Err(error) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(error)));
                }
            }
            let accepted = context.selected.work.as_ref().ok_or(RunError::Exhausted)?;
            if accepted.unread.is_some()
                || accepted.payload.is_some()
                || accepted.completion.is_some()
            {
                *state = None;
                return Ok(StepOutcome::Complete(Ok(None)));
            }
            let progress = context
                .selected
                .work
                .as_mut()
                .and_then(|accepted| accepted.progress.as_mut())
                .ok_or(RunError::Exhausted)?;
            let memory = BoundMemory::new(
                context.memory,
                (context.sideband)(DmaAccess {
                    channel: work.channel,
                    context: ContextId::new(progress.command.sg_context),
                }),
            );
            let span = progress.prepare(&memory);
            if progress.status.halts() {
                let completion = crate::Completion {
                    work,
                    response: progress.response(),
                };
                *state = None;
                return Ok(StepOutcome::Complete(
                    context.complete(completion).map(|()| None),
                ));
            }
            let Some((source, destination, length)) = span else {
                return Ok(StepOutcome::InProgress);
            };
            let chunk = Chunk {
                work,
                source,
                destination,
                length,
                source_context: progress.command.source.context,
                destination_context: progress.command.destination.context,
            };
            *state = None;
            Ok(StepOutcome::Complete(Ok(Some(chunk))))
        })
    }
}
