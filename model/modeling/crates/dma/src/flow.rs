use crate::channel::CopyState;
use crate::{COPY_CHUNK_BYTES, Work};
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::dma::{ChannelId, Command, ContextId, Response};

/// Non-owning observation of a channel head, refreshed before descriptor acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSnapshot {
    pub(crate) work: Work,
    pub(crate) consumer: u32,
    pub(crate) address: u64,
}

/// Accepted descriptor transferred into range validation.
#[derive(Debug)]
pub struct FetchedCommand {
    pub(crate) work: Work,
    pub(crate) command: Command,
}

impl FetchedCommand {
    /// The accepted immutable descriptor.
    pub const fn command(&self) -> &Command {
        &self.command
    }
    /// Its response-ring owner.
    pub const fn channel(&self) -> ChannelId {
        self.work.channel()
    }
}

/// Validated command traversal transferred between finite chunk passes.
#[derive(Debug)]
pub struct CopyProgress {
    pub(crate) work: Work,
    pub(crate) progress: CopyState,
}

/// A new traversal pass or the concrete chunk retained at a read fault.
#[derive(Debug)]
pub struct ChunkInput {
    pub(crate) work: Work,
    pub(crate) retry: Option<Chunk>,
}

impl ChunkInput {
    pub(crate) const fn next(work: Work) -> Self {
        Self { work, retry: None }
    }
    pub(crate) const fn retry(chunk: Chunk) -> Self {
        Self {
            work: chunk.work,
            retry: Some(chunk),
        }
    }
    /// Channel whose retained traversal or read checkpoint is requested.
    pub const fn channel(&self) -> ChannelId {
        self.work.channel()
    }
}

/// Bounded source/destination span and its authoritative command traversal.
#[derive(Debug)]
pub struct Chunk {
    pub(crate) work: Work,
    pub(crate) source: MemoryAddress,
    pub(crate) destination: MemoryAddress,
    pub(crate) length: usize,
    pub(crate) source_context: u32,
    pub(crate) destination_context: u32,
}

impl Chunk {
    /// Source byte address for this interaction.
    pub const fn source(&self) -> MemoryAddress {
        self.source
    }
    /// Destination byte address for this interaction.
    pub const fn destination(&self) -> MemoryAddress {
        self.destination
    }
    /// Number of payload bytes in this interaction.
    pub const fn len(&self) -> usize {
        self.length
    }
    /// Whether the command requires no payload interaction.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
    /// Source address-space selector.
    pub const fn source_context(&self) -> ContextId {
        ContextId::new(self.source_context)
    }
    /// Destination address-space selector.
    pub const fn destination_context(&self) -> ContextId {
        ContextId::new(self.destination_context)
    }
}

/// Successfully read bytes, owned by the dataflow until write or fault parking.
#[derive(Debug)]
pub struct Payload {
    pub(crate) chunk: Chunk,
    pub(crate) bytes: [u8; COPY_CHUNK_BYTES],
}

impl Payload {
    /// The bounded transfer descriptor.
    pub const fn chunk(&self) -> &Chunk {
        &self.chunk
    }
    /// Active payload bytes, with length fixed by traversal.
    pub fn bytes(&self) -> Result<&[u8], crate::Error> {
        self.bytes
            .get(..self.chunk.length)
            .ok_or(crate::Error::Routing)
    }
    /// Transform the actual bytes consumed by the write stage.
    pub fn bytes_mut(&mut self) -> Result<&mut [u8], crate::Error> {
        self.bytes
            .get_mut(..self.chunk.length)
            .ok_or(crate::Error::Routing)
    }
}

/// Response value and generation-bound response destination.
#[derive(Debug, PartialEq, Eq)]
pub struct Completion {
    pub(crate) work: Work,
    pub(crate) response: Response,
}

impl Completion {
    /// Response produced by command execution.
    pub const fn response(&self) -> &Response {
        &self.response
    }
    /// Destination response-ring owner.
    pub const fn channel(&self) -> ChannelId {
        self.work.channel()
    }
}
