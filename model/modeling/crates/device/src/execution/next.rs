use super::{COPY_CHUNK_BYTES, CopyState, Transfer};
use crate::{Device, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress};
use pdma_model_contract::ResponseStatus;

/// Load endpoint segments and select one bounded payload chunk.
#[derive(Debug, Default)]
pub struct NextChunk;

impl<M: MemoryAccess> Stage<Device<M>> for NextChunk {
    type Input = CopyState;
    type Output = Transfer;
    type Error = Error;
    type Wait = ();
    type State = Option<CopyState>;
    fn start(input: CopyState) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        context: &mut Device<M>,
        state: &mut Self::State,
    ) -> StepOutcome<Transfer, Error, ()> {
        if context.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        let Some(progress) = state.as_mut() else {
            return StepOutcome::Exhausted;
        };
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
                    match cursor.next_segment(&context.memory) {
                        Ok(Some(_)) => {}
                        Ok(None) => progress.status = ResponseStatus::LengthMismatch,
                        Err(status) => progress.status = status,
                    }
                    return StepOutcome::Progressed;
                }
                let Some(address) = progress.source.address() else {
                    progress.status = ResponseStatus::ReadFault;
                    return StepOutcome::Progressed;
                };
                source = address;
                let Some(address) = progress.destination.address() else {
                    progress.status = ResponseStatus::WriteFault;
                    return StepOutcome::Progressed;
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
            | ResponseStatus::ReadFault
            | ResponseStatus::WriteFault
            | ResponseStatus::LengthMismatch
            | ResponseStatus::ListReadFault => {}
        }
        state.take().map_or(StepOutcome::Exhausted, |progress| {
            StepOutcome::Complete(Ok(Transfer {
                progress,
                source: MemoryAddress::new(source),
                destination: MemoryAddress::new(destination),
                length,
                bytes: [0; COPY_CHUNK_BYTES],
            }))
        })
    }
}
