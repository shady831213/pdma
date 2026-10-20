mod cursor;
mod traversal;
mod validation;

use super::{CopyState, CopyStatus, RangeCheck};
use crate::Channel;
pub(super) use cursor::Cursor;
use pdma_model_contract::dma::{Command, CompletionInfo, Response, ResponseInfoValues};

/// Scratch space per copy interaction, not a protocol transfer-length limit.
pub const COPY_CHUNK_BYTES: usize = 256;

impl CopyStatus {
    pub(crate) const fn halts(self) -> bool {
        match self {
            Self::Success => false,
            Self::InvalidLength
            | Self::ReadError
            | Self::WriteError
            | Self::LengthMismatch
            | Self::ListReadError => true,
        }
    }
}

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
    const fn new(scan: &RangeCheck, status: CopyStatus) -> Self {
        let command = scan.command;
        Self {
            source: Cursor::new(command.source, command.copy_length),
            destination: Cursor::new(command.destination, command.copy_length),
            command,
            id: scan.id,
            flags: scan.flags,
            remaining: command.copy_length,
            bytes_completed: 0,
            status,
        }
    }

    pub(crate) const fn response(&self) -> Response {
        self.response_with(self.status)
    }

    pub(crate) const fn response_with(&self, status: CopyStatus) -> Response {
        let progress = CompletionInfo {
            bytes_completed: self.bytes_completed,
        };
        let info = match status {
            CopyStatus::Success => ResponseInfoValues::Success(progress),
            CopyStatus::InvalidLength => ResponseInfoValues::InvalidLength(progress),
            CopyStatus::ReadError => ResponseInfoValues::ReadError(progress),
            CopyStatus::WriteError => ResponseInfoValues::WriteError(progress),
            CopyStatus::LengthMismatch => ResponseInfoValues::LengthMismatch(progress),
            CopyStatus::ListReadError => ResponseInfoValues::ListReadError(progress),
        };
        Response {
            flags: self.flags,
            id: self.id,
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
