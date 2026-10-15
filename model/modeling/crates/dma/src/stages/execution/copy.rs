use crate::channel::Continuation;
use crate::{COPY_CHUNK_BYTES, Chunk, Completion, ExecutionContext, MemoryFailure, Payload};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::ResponseStatus;

/// Reads a concrete bounded chunk and transfers its bytes to the next stage.
#[derive(Debug, Default)]
pub struct ReadCopy;

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for ReadCopy
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Option<Chunk>;
    type Output = Option<Payload>;
    type Error = crate::ExecutionError;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, crate::ExecutionError> {
        let Some(chunk) = state.as_ref().ok_or(RunError::Exhausted)? else {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        match context.ready(chunk.work) {
            Ok(true) => {}
            Ok(false) => return Ok(StepOutcome::InProgress),
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error.into())));
            }
        }
        let mut bytes = [0; COPY_CHUNK_BYTES];
        if chunk.length > 0 {
            let output = bytes.get_mut(..chunk.length).ok_or(RunError::Exhausted)?;
            if let Err(error) = context.memory_for(chunk.work, chunk.source_context).read(
                chunk.source,
                MemoryAlignment::ONE,
                output,
            ) {
                match error.into() {
                    MemoryFailure::PageFault { .. } => {
                        let Some(Some(chunk)) = state.take() else {
                            return Err(RunError::Exhausted);
                        };
                        let result = context
                            .park(chunk.work, Continuation::Read(chunk))
                            .map(|()| None)
                            .map_err(crate::ExecutionError::from);
                        return Ok(StepOutcome::Complete(result));
                    }
                    MemoryFailure::Access => {
                        let progress = context
                            .selected
                            .work
                            .as_ref()
                            .and_then(|work| work.progress.as_ref())
                            .ok_or(RunError::Exhausted)?;
                        let completion = Completion {
                            work: chunk.work,
                            response: progress.response_with(ResponseStatus::ReadError),
                        };
                        *state = None;
                        return Ok(StepOutcome::Complete(Err(
                            crate::ExecutionError::CommandFailed(completion),
                        )));
                    }
                }
            }
        }
        let Some(Some(chunk)) = state.take() else {
            return Err(RunError::Exhausted);
        };
        Ok(StepOutcome::Complete(Ok(Some(Payload { chunk, bytes }))))
    }
}

/// Consumes actual payload bytes and commits traversal only after a successful write.
#[derive(Debug, Default)]
pub struct WriteCopy;

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for WriteCopy
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Option<Payload>;
    type Output = Option<Completion>;
    type Error = crate::ExecutionError;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, crate::ExecutionError> {
        let Some(payload) = state.as_ref().ok_or(RunError::Exhausted)? else {
            *state = None;
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        let work = payload.chunk.work;
        match context.ready(work) {
            Ok(true) => {}
            Ok(false) => return Ok(StepOutcome::InProgress),
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error.into())));
            }
        }
        if payload.chunk.length > 0 {
            let bytes = match payload.bytes() {
                Ok(bytes) => bytes,
                Err(error) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(error.into())));
                }
            };
            match context
                .memory_for(work, payload.chunk.destination_context)
                .write(payload.chunk.destination, MemoryAlignment::ONE, bytes)
            {
                Ok(()) => {
                    let progress = context
                        .selected
                        .work
                        .as_mut()
                        .and_then(|accepted| accepted.progress.as_mut())
                        .ok_or(RunError::Exhausted)?;
                    if let Err(error) = progress.commit(payload.chunk.length) {
                        *state = None;
                        return Ok(StepOutcome::Complete(Err(error.into())));
                    }
                }
                Err(error) => match error.into() {
                    MemoryFailure::PageFault { .. } => {
                        let Some(Some(payload)) = state.take() else {
                            return Err(RunError::Exhausted);
                        };
                        return Ok(StepOutcome::Complete(
                            context
                                .park(work, Continuation::Write(Box::new(payload)))
                                .map(|()| None)
                                .map_err(crate::ExecutionError::from),
                        ));
                    }
                    MemoryFailure::Access => {
                        let progress = context
                            .selected
                            .work
                            .as_ref()
                            .and_then(|accepted| accepted.progress.as_ref())
                            .ok_or(RunError::Exhausted)?;
                        let completion = Completion {
                            work,
                            response: progress.response_with(ResponseStatus::WriteError),
                        };
                        *state = None;
                        return Ok(StepOutcome::Complete(Ok(Some(completion))));
                    }
                },
            }
        }
        *state = None;
        let accepted = context.selected.work.as_mut().ok_or(RunError::Exhausted)?;
        let progress = accepted.progress.as_ref().ok_or(RunError::Exhausted)?;
        if progress.remaining > 0 {
            accepted.continuation = Some(Continuation::Next);
            Ok(StepOutcome::Complete(Ok(None)))
        } else {
            Ok(StepOutcome::Complete(Ok(Some(Completion {
                work,
                response: progress.response(),
            }))))
        }
    }
}
