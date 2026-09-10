use super::{CopyState, Transfer};
use crate::{Device, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::{Response, ResponseStatus};
use std::ops::ControlFlow;

/// Read one payload chunk into retained operation state.
#[derive(Debug, Default)]
pub struct ReadCopy;

impl<M: MemoryAccess> Stage<Device<M>> for ReadCopy {
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
        context: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<Transfer, Error, ()> {
        if context.channel.halted() {
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
                transfer.progress.status = match transfer.bytes.get_mut(..transfer.length) {
                    Some(bytes) => {
                        match context
                            .memory
                            .read(transfer.source, MemoryAlignment::ONE, bytes)
                        {
                            Ok(()) => ResponseStatus::Success,
                            Err(_) => ResponseStatus::ReadFault,
                        }
                    }
                    None => ResponseStatus::InvalidLength,
                };
            }
            ResponseStatus::Success
            | ResponseStatus::InvalidLength
            | ResponseStatus::ReadFault
            | ResponseStatus::WriteFault
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadFault => {}
        }
        StepOutcome::Complete(Ok(transfer))
    }
}

/// Commit one chunk, then feed traversal back or finish the command.
#[derive(Debug, Default)]
pub struct WriteCopy;

impl<M: MemoryAccess> Stage<Device<M>> for WriteCopy {
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
        context: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<Self::Output, Error, ()> {
        if context.channel.halted() {
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
                            return StepOutcome::Complete(Ok(ControlFlow::Break(
                                transfer.progress.response(),
                            )));
                        }
                        match context.memory.write(
                            transfer.destination,
                            MemoryAlignment::ONE,
                            bytes,
                        ) {
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
                            Err(_) => transfer.progress.status = ResponseStatus::WriteFault,
                        }
                    }
                    _ => transfer.progress.status = ResponseStatus::InvalidLength,
                }
            }
            ResponseStatus::InvalidLength
            | ResponseStatus::ReadFault
            | ResponseStatus::WriteFault
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadFault => {}
        }
        StepOutcome::Complete(Ok(ControlFlow::Break(transfer.progress.response())))
    }
}
