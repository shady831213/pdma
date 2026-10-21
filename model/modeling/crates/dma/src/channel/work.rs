mod cursor;
mod traversal;
mod validation;

use super::CopyState;
use crate::Channel;
pub(super) use cursor::Cursor;
use pdma_model_contract::dma::{
    Command, CompletionInfo, Response, ResponseInfoValues, ResponseStatus,
};

/// Scratch space per copy interaction, not a protocol transfer-length limit.
pub const COPY_CHUNK_BYTES: usize = 256;

impl CopyState {
    pub(crate) fn commit(&mut self, count: usize) -> Result<(), crate::Error> {
        let count = u64::try_from(count).map_err(|_| crate::Error::Routing)?;
        if count > self.remaining
            || count > self.source.remaining()
            || count > self.destination.remaining()
        {
            return Err(crate::Error::Routing);
        }
        self.source.advance(count);
        self.destination.advance(count);
        self.remaining -= count;
        self.bytes_completed += count;
        Ok(())
    }
    const fn new(command: Command, status: ResponseStatus) -> Self {
        Self {
            source: Cursor::new(command.source, command.copy_length),
            destination: Cursor::new(command.destination, command.copy_length),
            command,
            remaining: command.copy_length,
            bytes_completed: 0,
            status,
        }
    }

    pub(crate) const fn response(&self) -> Response {
        self.response_with(self.status)
    }

    pub(crate) const fn response_with(&self, status: ResponseStatus) -> Response {
        let progress = CompletionInfo {
            bytes_completed: self.bytes_completed,
        };
        let info = match status {
            ResponseStatus::Success => ResponseInfoValues::Success(progress),
            ResponseStatus::InvalidLength => ResponseInfoValues::InvalidLength(progress),
            ResponseStatus::ReadError => ResponseInfoValues::ReadError(progress),
            ResponseStatus::WriteError => ResponseInfoValues::WriteError(progress),
            ResponseStatus::LengthMismatch => ResponseInfoValues::LengthMismatch(progress),
            ResponseStatus::ListReadError => ResponseInfoValues::ListReadError(progress),
        };
        Response {
            flags: self.command.flags,
            id: self.command.id,
            status: info.tag(),
            info,
        }
    }
}

impl Channel {
    pub(crate) fn accepted_command(&self) -> Option<&Command> {
        self.work.as_ref().map(|work| &work.command)
    }
}
