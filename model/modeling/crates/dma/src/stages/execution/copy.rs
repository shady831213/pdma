use crate::{COPY_CHUNK_BYTES, Chunk, Completion, ExecutionContext, Flow, MemoryFailure, Payload};
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use hw_model_pipeline::{RunError, Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::EventCode;

/// Reads a concrete bounded chunk and transfers its bytes to the next stage.
#[derive(Debug, Default)]
pub struct ReadCopy {
    state: Option<Flow<Chunk>>,
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for ReadCopy
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Flow<Chunk>;
    type Output = Flow<Payload>;
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
            if state.is_none() {
                match context.checkpoint_ready() {
                    Ok(true) => {
                        *state = context
                            .selected
                            .work
                            .as_mut()
                            .and_then(|accepted| accepted.unread.take());
                    }
                    Ok(false) => {}
                    Err(error) => {
                        *state = None;
                        return Ok(StepOutcome::Complete(Err(error)));
                    }
                }
            }
            let Some(chunk) = state.as_ref() else {
                return Ok(StepOutcome::Complete(Ok(None)));
            };
            match context.ready(chunk.work) {
                Ok(true) => {}
                Ok(false) => return Ok(StepOutcome::InProgress),
                Err(error) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(error)));
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
                            let Some(chunk) = state.take() else {
                                return Err(RunError::Exhausted);
                            };
                            let work = chunk.work;
                            context
                                .selected
                                .work
                                .as_mut()
                                .ok_or(RunError::Exhausted)?
                                .unread = Some(chunk);
                            let result = context.park(work).map(|()| None);
                            return Ok(StepOutcome::Complete(result));
                        }
                        MemoryFailure::Access => {
                            let progress = context
                                .selected
                                .work
                                .as_ref()
                                .and_then(|work| work.progress.as_ref())
                                .ok_or(RunError::Exhausted)?;
                            let event = progress.event_with(EventCode::ReadError);
                            *state = None;
                            return Ok(StepOutcome::Complete(Err(context.report(&event))));
                        }
                    }
                }
            }
            let Some(chunk) = state.take() else {
                return Err(RunError::Exhausted);
            };
            Ok(StepOutcome::Complete(Ok(Some(Payload { chunk, bytes }))))
        })
    }
}

/// Consumes actual payload bytes and commits traversal only after a successful write.
#[derive(Debug, Default)]
pub struct WriteCopy {
    state: Option<Flow<Payload>>,
}

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for WriteCopy
where
    M::Error: Into<MemoryFailure>,
{
    type Input = Flow<Payload>;
    type Output = Flow<Completion>;
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
            if state.is_none() {
                match context.checkpoint_ready() {
                    Ok(true) => {
                        *state = context
                            .selected
                            .work
                            .as_mut()
                            .and_then(|accepted| accepted.payload.take())
                            .map(|payload| *payload);
                    }
                    Ok(false) => {}
                    Err(error) => {
                        *state = None;
                        return Ok(StepOutcome::Complete(Err(error)));
                    }
                }
            }
            let Some(payload) = state.as_ref() else {
                return Ok(StepOutcome::Complete(Ok(None)));
            };
            let work = payload.chunk.work;
            match context.ready(work) {
                Ok(true) => {}
                Ok(false) => return Ok(StepOutcome::InProgress),
                Err(error) => {
                    *state = None;
                    return Ok(StepOutcome::Complete(Err(error)));
                }
            }
            if payload.chunk.length > 0 {
                let bytes = match payload.bytes() {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        *state = None;
                        return Ok(StepOutcome::Complete(Err(error)));
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
                            return Ok(StepOutcome::Complete(Err(error)));
                        }
                    }
                    Err(error) => match error.into() {
                        MemoryFailure::PageFault { .. } => {
                            let Some(payload) = state.take() else {
                                return Err(RunError::Exhausted);
                            };
                            context
                                .selected
                                .work
                                .as_mut()
                                .ok_or(RunError::Exhausted)?
                                .payload = Some(Box::new(payload));
                            return Ok(StepOutcome::Complete(context.park(work).map(|()| None)));
                        }
                        MemoryFailure::Access => {
                            let progress = context
                                .selected
                                .work
                                .as_ref()
                                .and_then(|accepted| accepted.progress.as_ref())
                                .ok_or(RunError::Exhausted)?;
                            let event = progress.event_with(EventCode::WriteError);
                            *state = None;
                            return Ok(StepOutcome::Complete(Err(context.report(&event))));
                        }
                    },
                }
            }
            *state = None;
            let accepted = context.selected.work.as_mut().ok_or(RunError::Exhausted)?;
            let progress = accepted.progress.as_ref().ok_or(RunError::Exhausted)?;
            Ok(StepOutcome::Complete(Ok(
                (progress.remaining == 0).then_some(Completion { work })
            )))
        })
    }
}
