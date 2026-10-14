use crate::{Channel, Dma, Error};
use hardware_interface_runtime::{BoundMemory, MemoryAccessError, MemoryAddress};
use pdma_model_contract::dma::{ChannelId, ContextId, DmaAccess};

/// Memory-port failures understood by DMA, independent of backend diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryFailure {
    /// The access failed without a repairable translation miss.
    Access,
    /// A payload mapping can be repaired and the retained command resumed.
    /// Converting a failed write to this variant guarantees that the call
    /// committed no externally visible bytes. Earlier accesses are unaffected.
    PageFault {
        /// First missing virtual byte within the requested access.
        address: MemoryAddress,
    },
}

impl From<MemoryAccessError> for MemoryFailure {
    fn from(error: MemoryAccessError) -> Self {
        match error {
            MemoryAccessError::AddressOverflow { .. }
            | MemoryAccessError::Misaligned { .. }
            | MemoryAccessError::Unmapped { .. }
            | MemoryAccessError::CrossesRegion { .. } => Self::Access,
        }
    }
}

impl<M, S> Dma<M, S> {
    /// Borrows one channel and shared hardware resources for a single interaction.
    pub fn channel_context(&mut self, id: ChannelId) -> Result<ChannelContext<'_, M, S>, Error> {
        let channel = self.channels.get_mut(id.index()).ok_or(Error::Unmapped)?;
        let access = DmaAccess {
            channel: id,
            context: ContextId::new(channel.registers.values.ring_context.raw()),
        };
        Ok(ChannelContext {
            channel,
            memory: BoundMemory::new(&self.memory, (self.sideband)(access)),
            backing: &self.memory,
            sideband: self.sideband,
            id,
        })
    }
}

/// One interaction's selected channel; no borrow escapes a stage step.
#[derive(Debug)]
pub struct ChannelContext<'a, M, S = ()> {
    /// The channel whose operation is being advanced.
    pub channel: &'a mut Channel,
    /// Memory access restricted to this channel's selected address space.
    pub memory: BoundMemory<'a, M, S>,
    backing: &'a M,
    sideband: fn(DmaAccess) -> S,
    id: ChannelId,
}

impl<M, S> ChannelContext<'_, M, S> {
    /// Selects a descriptor's address space without exposing resolved access attributes.
    pub fn memory_for(&self, context: u32) -> BoundMemory<'_, M, S> {
        BoundMemory::new(
            self.backing,
            (self.sideband)(DmaAccess {
                channel: self.id,
                context: ContextId::new(context),
            }),
        )
    }
}
