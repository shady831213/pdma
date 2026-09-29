use super::{CopyState, Transfer};
use crate::{ChannelContext, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{PageFaultKindsAccess, Response, ResponseStatus};
use std::ops::ControlFlow;

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
    type Wait = ();
    type State = Option<Transfer>;
    fn start(input: Transfer) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepOutcome<Transfer, Error, ()> {
        if context.channel.resetting() {
            return StepOutcome::Complete(Err(Error::Reset));
        }
        if context.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        let Some(mut transfer) = state.take() else {
            return StepOutcome::Exhausted;
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
                            Err(error) => transfer
                                .progress
                                .fail_access(error.into(), PageFaultKindsAccess::SourceRead),
                        }
                    }
                    None => transfer.progress.status = ResponseStatus::InvalidLength,
                }
            }
            ResponseStatus::Success
            | ResponseStatus::PageFault
            | ResponseStatus::InvalidLength
            | ResponseStatus::ReadError
            | ResponseStatus::WriteError
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadError => {}
        }
        StepOutcome::Complete(Ok(transfer))
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
    type Output = ControlFlow<Response, CopyState>;
    type Error = Error;
    type Wait = ();
    type State = Option<Transfer>;
    fn start(input: Transfer) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Error, ()> {
        if context.channel.resetting() {
            return StepOutcome::Complete(Err(Error::Reset));
        }
        if context.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        let Some(mut transfer) = state.take() else {
            return StepOutcome::Exhausted;
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
                            return StepOutcome::Complete(
                                transfer.progress.response().map(ControlFlow::Break),
                            );
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
                                    return StepOutcome::Complete(Ok(ControlFlow::Continue(
                                        transfer.progress,
                                    )));
                                }
                            }
                            Err(error) => transfer
                                .progress
                                .fail_access(error.into(), PageFaultKindsAccess::DestinationWrite),
                        }
                    }
                    _ => transfer.progress.status = ResponseStatus::InvalidLength,
                }
            }
            ResponseStatus::InvalidLength
            | ResponseStatus::PageFault
            | ResponseStatus::ReadError
            | ResponseStatus::WriteError
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadError => {}
        }
        StepOutcome::Complete(transfer.progress.response().map(ControlFlow::Break))
    }
}
