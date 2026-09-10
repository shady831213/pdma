//! Minimal single-channel DMA hardware.

mod channel;
mod error;
mod execution;
mod stages;

pub use channel::{Channel, CommandRing, ResponseRing};
pub use error::{Error, Fault};
pub use execution::{
    COPY_CHUNK_BYTES, CheckLengths, CopyState, LengthCheck, NextChunk, ReadCopy, Transfer,
    WriteCopy,
};
pub use stages::{CopyLoop, Execution, Fetch, Pipeline, Publish, UpdateIrq, pipeline};

/// Hardware context; the assembler chooses and owns the memory backing.
#[derive(Debug)]
pub struct Device<M> {
    /// The single channel and its register-owning rings.
    pub channel: Channel,
    /// Injected memory access capability, never a device-selected backend.
    pub memory: M,
}

impl<M> Device<M> {
    /// Creates disabled hardware without allocating any external memory.
    pub fn new(memory: M) -> Self {
        Self {
            channel: Channel::default(),
            memory,
        }
    }

    /// Current aggregate interrupt level, independent of host delivery.
    pub const fn irq(&self) -> bool {
        self.channel.irq
    }

    /// Reads an exact register leaf in the generated device map.
    pub fn read(&self, offset: usize, destination: &mut [u8]) -> Result<(), Error> {
        self.channel.read(offset, destination)
    }

    /// Writes a register and applies the owning channel's immediate side effects.
    pub fn write(&mut self, offset: usize, source: &[u8]) -> Result<(), Error> {
        self.channel.write(offset, source)
    }
}
