mod copy;
mod cursor;
mod next;
mod validate;

pub use copy::{ReadCopy, WriteCopy};
pub use next::NextChunk;
pub use validate::{CheckRanges, RangeCheck};

use cursor::Cursor;
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::dma::{
    Command, CompletionInfo, Response, ResponseInfoValues, ResponseStatus,
};

/// Scratch space per copy interaction, not a protocol transfer-length limit.
pub const COPY_CHUNK_BYTES: usize = 256;

/// Fresh descriptor or hardware traversal retained between service transactions.
#[derive(Debug)]
pub enum CommandInput {
    /// Descriptor whose endpoint ranges have not been checked.
    Fresh(Command),
    /// Validated traversal at the next chunk boundary.
    Progress(CopyState),
}

impl CommandInput {
    /// Descriptor metadata shared by fresh and continuing service transactions.
    pub const fn command(&self) -> &Command {
        match self {
            Self::Fresh(command) => command,
            Self::Progress(progress) => &progress.command,
        }
    }
}

/// Retained traversal of one command's two byte streams.
#[derive(Debug)]
pub struct CopyState {
    command: Command,
    source: Cursor,
    destination: Cursor,
    remaining: u64,
    bytes_completed: u64,
    status: ResponseStatus,
}

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

    const fn response(&self) -> Response {
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

/// One retained payload chunk between the composable read and write stages.
#[derive(Debug)]
pub struct Transfer {
    /// Command progress returned to the next copy iteration.
    pub progress: CopyState,
    /// Current source byte address.
    pub source: MemoryAddress,
    /// Current destination byte address.
    pub destination: MemoryAddress,
    /// Active prefix of the scratch buffer.
    pub length: usize,
    /// Owned bytes available to an inserted payload transform.
    pub bytes: [u8; COPY_CHUNK_BYTES],
}
