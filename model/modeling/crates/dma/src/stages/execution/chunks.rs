use crate::{Chunk, ChunkInput, ExecutionContext};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{BoundMemory, MemoryAccess};
use pdma_model_contract::dma::{ContextId, DmaAccess};

/// Selects the next bounded chunk using the channel-owned traversal cursors.
#[derive(Debug, Default)]
pub struct PrepareChunk;

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for PrepareChunk {
    type Input = Option<ChunkInput>;
    type Output = Option<Chunk>;
    type Error = crate::Error;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, crate::Error> {
        let Some(input) = state.as_mut().ok_or(RunError::Exhausted)? else {
            *state = None;
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
        if accepted.unread.is_some() || accepted.payload.is_some() || accepted.completion.is_some()
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
    }
}
