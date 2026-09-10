mod copy;
mod cursor;
mod next;
mod validate;

pub use copy::{ReadCopy, WriteCopy};
pub use next::NextChunk;
pub use validate::{CheckLengths, LengthCheck};

use cursor::Cursor;
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::{Command, Response, ResponseStatus};

/// Scratch space per copy interaction, not a protocol transfer-length limit.
pub const COPY_CHUNK_BYTES: usize = 256;

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
    const fn new(command: Command, length: u64, status: ResponseStatus) -> Self {
        Self {
            source: Cursor::new(command.source),
            destination: Cursor::new(command.destination),
            command,
            remaining: length,
            bytes_completed: 0,
            status,
        }
    }

    const fn response(&self) -> Response {
        Response {
            id: self.command.id,
            status: self.status,
            bytes_completed: self.bytes_completed,
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
