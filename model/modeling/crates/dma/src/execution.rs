mod copy;
mod cursor;
mod next;
mod validate;

pub use copy::{ReadCopy, WriteCopy};
pub use next::NextChunk;
pub use validate::{CheckRanges, RangeCheck};

use cursor::Cursor;
use hardware_interface_runtime::{MemoryAccessError, MemoryAddress};
use pdma_model_contract::dma::{
    Command, CompletionInfo, PageFaultInfo, PageFaultKindsAccess, Response, ResponseInfoValues,
    ResponseStatus,
};

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
    fault: Option<(MemoryAddress, PageFaultKindsAccess)>,
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
            fault: None,
        }
    }

    fn response(&self) -> Result<Response, crate::Error> {
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
            ResponseStatus::PageFault => {
                let (address, access) = self.fault.ok_or(crate::Error::Routing)?;
                ResponseInfoValues::PageFault(PageFaultInfo::from_positions(
                    self.bytes_completed,
                    address.raw(),
                    access,
                    self.source.position(),
                    self.destination.position(),
                ))
            }
        };
        Ok(Response {
            flags: self.command.flags,
            id: self.command.id,
            status: info.tag(),
            info,
        })
    }

    const fn fail_access(&mut self, error: MemoryAccessError, access: PageFaultKindsAccess) {
        self.status = match error {
            MemoryAccessError::PageFault { address } => {
                self.fault = Some((address, access));
                ResponseStatus::PageFault
            }
            MemoryAccessError::AddressOverflow { .. }
            | MemoryAccessError::Misaligned { .. }
            | MemoryAccessError::Unmapped { .. }
            | MemoryAccessError::CrossesRegion { .. } => match access {
                PageFaultKindsAccess::SourceRead => ResponseStatus::ReadError,
                PageFaultKindsAccess::DestinationWrite => ResponseStatus::WriteError,
            },
        };
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
