mod chunks;
mod completion;
mod copy;
mod validate;

pub use chunks::PrepareChunk;
pub use completion::CompletionInput;
pub use copy::{ReadCopy, WriteCopy};
pub use validate::CheckRanges;
pub(super) use validate::Install;

use crate::{ChunkInput, Completion, Error, ExecutionContext, Flow};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use std::convert::Infallible;

/// Executes successive chunks of one accepted command through the same child graph.
#[derive(Debug)]
pub struct CommandExecution<C> {
    chunks: C,
    input: Option<Flow<ChunkInput>>,
    busy: bool,
}

impl<C> CommandExecution<C> {
    /// Keeps command continuation outside the supplied chunk-level composition.
    pub const fn new(chunks: C) -> Self {
        Self {
            chunks,
            input: None,
            busy: false,
        }
    }
}

impl<M, SB, C> Stage<ExecutionContext<'_, M, SB>> for CommandExecution<C>
where
    for<'a> C: Stage<
            ExecutionContext<'a, M, SB>,
            Input = Flow<ChunkInput>,
            Output = Flow<Completion>,
            Error = Infallible,
        >,
{
    type Input = Flow<ChunkInput>;
    type Output = Flow<Completion>;
    type Error = Infallible;

    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if !self.busy {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.input = Some(input);
            self.busy = true;
        }
        let output = match self.chunks.step(context, &mut self.input)? {
            StepOutcome::InProgress => return Ok(StepOutcome::InProgress),
            StepOutcome::Complete(Ok(output)) => output,
            StepOutcome::Complete(Err(error)) => match error {},
        };
        let next = output.status().and_then(|()| {
            if output.data().is_some() {
                return Ok(None);
            }
            let Some(work) = output.work else {
                return Ok(None);
            };
            let selected = context.select(work)?;
            selected.lifecycle()?;
            if *selected.active != Some(work) {
                return Ok(None);
            }
            let accepted = selected.selected.work.as_ref().ok_or(Error::Reset)?;
            if accepted.completion.is_some() {
                return Ok(None);
            }
            let progress = accepted.progress.as_ref().ok_or(Error::Routing)?;
            if progress.remaining == 0 {
                return Err(Error::Routing);
            }
            Ok(Some(ChunkInput::next(work)))
        });
        match next {
            Ok(Some(next)) => {
                self.input = Some(Flow::new(output.work, Ok(Some(next))));
                Ok(StepOutcome::InProgress)
            }
            Ok(None) => {
                self.input = None;
                self.busy = false;
                Ok(StepOutcome::Complete(Ok(output)))
            }
            Err(error) => {
                self.input = None;
                self.busy = false;
                Ok(StepOutcome::Complete(Ok(output.stop(error))))
            }
        }
    }
}
