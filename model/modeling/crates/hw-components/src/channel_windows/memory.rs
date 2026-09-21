use super::ChannelWindows;
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{ChannelId, FunctionId};

/// Translates channel metadata to its register-owned Function before forwarding memory access.
#[derive(Debug)]
pub struct ChannelMemoryAccess<'a, M: ?Sized> {
    windows: &'a ChannelWindows,
    downstream: &'a M,
}

impl ChannelWindows {
    /// Borrows the owner bank and a Function-tagged downstream port without owning its backing.
    pub const fn memory<'a, M: ?Sized>(&'a self, downstream: &'a M) -> ChannelMemoryAccess<'a, M> {
        ChannelMemoryAccess {
            windows: self,
            downstream,
        }
    }
}

impl<M: ?Sized> ChannelMemoryAccess<'_, M> {
    fn owner(
        &self,
        channel: ChannelId,
        address: MemoryAddress,
    ) -> Result<FunctionId, MemoryAccessError> {
        self.windows
            .owner(channel)
            .ok_or(MemoryAccessError::Unmapped { address })
    }
}

impl<M: MemoryAccess<FunctionId> + ?Sized> MemoryAccess<ChannelId> for ChannelMemoryAccess<'_, M> {
    fn validate_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        let function = self.owner(*channel, address)?;
        self.downstream
            .validate_with_sideband(&function, address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        let function = self.owner(*channel, address)?;
        self.downstream
            .read_with_sideband(&function, address, alignment, bytes)
    }

    fn write_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        let function = self.owner(*channel, address)?;
        self.downstream
            .write_with_sideband(&function, address, alignment, bytes)
    }
}
