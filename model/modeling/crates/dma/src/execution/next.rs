use super::{COPY_CHUNK_BYTES, CopyState, Transfer};
use crate::{ChannelContext, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};
use hardware_interface_runtime::MemoryAccess;
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::dma::ResponseStatus;

/// Load endpoint segments and select one bounded payload chunk.
#[derive(Debug, Default)]
pub struct NextChunk;

impl<M: MemoryAccess<SB>, SB> Stage<ChannelContext<'_, M, SB>> for NextChunk {
    type Input = CopyState;
    type Output = Transfer;
    type Error = Error;
    type Wait = ();
    type State = CopyState;
    fn start(input: CopyState) -> Self::State {
        input
    }

    fn step(
        &mut self,
        context: &mut ChannelContext<'_, M, SB>,
        mut progress: Self::State,
    ) -> StepOutcome<Self::State, Transfer, Error, ()> {
        if context.channel.resetting() {
            return StepOutcome::Complete(Err(Error::Reset));
        }
        if context.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(progress, ());
        }
        let mut source = 0;
        let mut destination = 0;
        let mut length = 0;
        match progress.status {
            ResponseStatus::Success if progress.remaining > 0 => {
                let cursor = if progress.source.remaining() == 0 {
                    Some(&mut progress.source)
                } else if progress.destination.remaining() == 0 {
                    Some(&mut progress.destination)
                } else {
                    None
                };
                if let Some(cursor) = cursor {
                    match cursor.next_segment(&context.memory_for(progress.command.sg_context)) {
                        Ok(true) => {}
                        Ok(false) => progress.status = ResponseStatus::LengthMismatch,
                        Err(status) => progress.status = status,
                    }
                    return StepOutcome::Progressed(progress);
                }
                let Some(address) = progress.source.address() else {
                    progress.status = ResponseStatus::ReadError;
                    return StepOutcome::Progressed(progress);
                };
                source = address;
                let Some(address) = progress.destination.address() else {
                    progress.status = ResponseStatus::WriteError;
                    return StepOutcome::Progressed(progress);
                };
                destination = address;
                let remaining = progress
                    .remaining
                    .min(progress.source.remaining())
                    .min(progress.destination.remaining());
                length = usize::try_from(remaining)
                    .unwrap_or(COPY_CHUNK_BYTES)
                    .min(COPY_CHUNK_BYTES);
            }
            ResponseStatus::Success
            | ResponseStatus::InvalidLength
            | ResponseStatus::ReadError
            | ResponseStatus::WriteError
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadError => {}
        }
        StepOutcome::Complete(Ok(Transfer {
            progress,
            source: MemoryAddress::new(source),
            destination: MemoryAddress::new(destination),
            length,
            bytes: [0; COPY_CHUNK_BYTES],
        }))
    }
}
