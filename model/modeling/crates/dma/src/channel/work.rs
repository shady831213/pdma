mod cursor;
mod payload;
mod traversal;
mod validation;

use super::{CopyState, Transfer};
use crate::Channel;
pub(super) use cursor::Cursor;
use pdma_model_contract::dma::{
    Command, CompletionInfo, Response, ResponseInfoValues, ResponseStatus,
};

/// Scratch space per copy interaction, not a protocol transfer-length limit.
pub const COPY_CHUNK_BYTES: usize = 256;

impl CopyState {
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
        let progress = CompletionInfo {
            bytes_completed: self.bytes_completed,
        };
        let info = match self.status {
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
