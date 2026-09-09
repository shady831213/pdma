use crate::{Device, Error};
use hardware_interface_pipeline::{Stage, StepOutcome};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{Command, MAX_COPY_BYTES, Response, ResponseStatus};

/// Request-owned data passed from the read stage into the write stage.
#[derive(Debug)]
pub struct Transfer {
    /// Descriptor accompanying the payload through composed execution stages.
    pub command: Command,
    /// Owned scratch payload; only `command.length` bytes are written on success.
    pub bytes: [u8; MAX_COPY_BYTES],
    /// Command-level result propagated into the response stage.
    pub status: ResponseStatus,
}

/// Read the bounded copy payload, propagating command errors as data.
#[derive(Debug, Default)]
pub struct ReadCopy;

impl<M: MemoryAccess> Stage<Device<M>> for ReadCopy {
    type Input = Command;
    type Output = Transfer;
    type Error = Error;
    type Wait = ();
    type State = Command;
    fn start(input: Command) -> Command {
        input
    }
    fn step(
        &mut self,
        context: &mut Device<M>,
        command: &mut Command,
    ) -> StepOutcome<Transfer, Error, ()> {
        if context.channel.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        let mut transfer = Transfer {
            command: *command,
            bytes: [0; MAX_COPY_BYTES],
            status: ResponseStatus::Success,
        };
        match usize::try_from(command.length)
            .ok()
            .and_then(|length| transfer.bytes.get_mut(..length))
        {
            None => transfer.status = ResponseStatus::InvalidLength,
            Some([]) => {}
            Some(destination) => {
                if context
                    .memory
                    .read(
                        MemoryAddress::new(command.source),
                        MemoryAlignment::ONE,
                        destination,
                    )
                    .is_err()
                {
                    transfer.status = ResponseStatus::ReadFault;
                }
            }
        }
        StepOutcome::Complete(Ok(transfer))
    }
}

/// Write the retained payload once and produce the response value.
#[derive(Debug, Default)]
pub struct WriteCopy;

impl<M: MemoryAccess> Stage<Device<M>> for WriteCopy {
    type Input = Transfer;
    type Output = Response;
    type Error = Error;
    type Wait = ();
    type State = Transfer;
    fn start(input: Transfer) -> Transfer {
        input
    }
    fn step(
        &mut self,
        context: &mut Device<M>,
        transfer: &mut Transfer,
    ) -> StepOutcome<Response, Error, ()> {
        if context.channel.halted() {
            return StepOutcome::Complete(Err(Error::Halted));
        }
        if !context.channel.enabled() {
            return StepOutcome::Pending(());
        }
        let status = match transfer.status {
            ResponseStatus::Success => match usize::try_from(transfer.command.length)
                .ok()
                .and_then(|length| transfer.bytes.get(..length))
            {
                None => ResponseStatus::InvalidLength,
                Some([]) => ResponseStatus::Success,
                Some(source) => match context.memory.write(
                    MemoryAddress::new(transfer.command.destination),
                    MemoryAlignment::ONE,
                    source,
                ) {
                    Ok(()) => ResponseStatus::Success,
                    Err(_) => ResponseStatus::WriteFault,
                },
            },
            ResponseStatus::InvalidLength
            | ResponseStatus::ReadFault
            | ResponseStatus::WriteFault => transfer.status,
        };
        StepOutcome::Complete(Ok(Response {
            id: transfer.command.id,
            status,
        }))
    }
}
