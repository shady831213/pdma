use super::Transfer;
use crate::{ChannelContext, Error, MemoryFailure};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{Response, ResponseStatus};

/// Read one payload chunk into retained operation state.
#[derive(Debug, Default)]
pub struct ReadCopy;

impl<M: MemoryAccess<SB>, SB> Stage<ChannelContext<'_, M, SB>> for ReadCopy
where
    M::Error: Into<crate::MemoryFailure>,
{
    type Input = Transfer;
    type Output = Transfer;
    type Error = Error;
    type State = Option<Transfer>;
    fn start(input: Transfer) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Transfer, Error> {
        if context.channel.resetting() {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Reset)));
        }
        if context.halted() {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Halted)));
        }
        if !context.channel.enabled() || context.channel.registers.values.fault_status.faulted() {
            return Ok(StepOutcome::InProgress);
        }
        let Some(mut transfer) = state.take() else {
            return Err(RunError::Exhausted);
        };
        match transfer.progress.status {
            ResponseStatus::Success if transfer.length > 0 => {
                match transfer.bytes.get_mut(..transfer.length) {
                    Some(bytes) => {
                        match context
                            .memory_for(transfer.progress.command.source.context)
                            .read(transfer.source, MemoryAlignment::ONE, bytes)
                        {
                            Ok(()) => {}
                            Err(error) => match error.into() {
                                MemoryFailure::PageFault { .. } => {
                                    *state = Some(transfer);
                                    context.channel.suspend();
                                    return Ok(StepOutcome::InProgress);
                                }
                                MemoryFailure::Access => {
                                    transfer.progress.status = ResponseStatus::ReadError;
                                }
                            },
                        }
                    }
                    None => transfer.progress.status = ResponseStatus::InvalidLength,
                }
            }
            ResponseStatus::Success
            | ResponseStatus::InvalidLength
            | ResponseStatus::ReadError
            | ResponseStatus::WriteError
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadError => {}
        }
        Ok(StepOutcome::Complete(Ok(transfer)))
    }
}

/// Commit one chunk, then feed traversal back or finish the command.
#[derive(Debug, Default)]
pub struct WriteCopy;

impl<M: MemoryAccess<SB>, SB> Stage<ChannelContext<'_, M, SB>> for WriteCopy
where
    M::Error: Into<crate::MemoryFailure>,
{
    type Input = Transfer;
    type Output = Option<Response>;
    type Error = Error;
    type State = Option<Transfer>;
    fn start(input: Transfer) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        if context.channel.resetting() {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Reset)));
        }
        if context.halted() {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Halted)));
        }
        if !context.channel.enabled() || context.channel.registers.values.fault_status.faulted() {
            return Ok(StepOutcome::InProgress);
        }
        let Some(mut transfer) = state.take() else {
            return Err(RunError::Exhausted);
        };
        match transfer.progress.status {
            ResponseStatus::Success => {
                match (
                    transfer.bytes.get(..transfer.length),
                    u64::try_from(transfer.length),
                ) {
                    (Some(bytes), Ok(count))
                        if count <= transfer.progress.remaining
                            && count <= transfer.progress.source.remaining()
                            && count <= transfer.progress.destination.remaining()
                            && (count > 0 || transfer.progress.remaining == 0) =>
                    {
                        if count == 0 {
                            return Ok(StepOutcome::Complete(Ok(Some(
                                transfer.progress.response(),
                            ))));
                        }
                        match context
                            .memory_for(transfer.progress.command.destination.context)
                            .write(transfer.destination, MemoryAlignment::ONE, bytes)
                        {
                            Ok(()) => {
                                transfer.progress.source.advance(count);
                                transfer.progress.destination.advance(count);
                                transfer.progress.remaining -= count;
                                transfer.progress.bytes_completed += count;
                                if transfer.progress.remaining > 0 {
                                    context.channel.progress = Some(transfer.progress);
                                    return Ok(StepOutcome::Complete(Ok(None)));
                                }
                            }
                            Err(error) => match error.into() {
                                MemoryFailure::PageFault { .. } => {
                                    *state = Some(transfer);
                                    context.channel.suspend();
                                    return Ok(StepOutcome::InProgress);
                                }
                                MemoryFailure::Access => {
                                    transfer.progress.status = ResponseStatus::WriteError;
                                }
                            },
                        }
                    }
                    _ => transfer.progress.status = ResponseStatus::InvalidLength,
                }
            }
            ResponseStatus::InvalidLength
            | ResponseStatus::ReadError
            | ResponseStatus::WriteError
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadError => {}
        }
        Ok(StepOutcome::Complete(Ok(Some(
            transfer.progress.response(),
        ))))
    }
}
