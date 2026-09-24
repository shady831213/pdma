use super::ControlChannel;
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::memory::MemoryIdentity;

#[doc = "Borrowed firmware Host-memory port using the independent PF control configuration."]
#[derive(Debug)]
pub struct ControlMemoryAccess<'a, M: ?Sized> {
    control: &'a ControlChannel,
    downstream: &'a M,
}

impl ControlChannel {
    #[doc = "Borrows a fabric port without taking ownership of backing memory."]
    pub const fn memory<'a, M: ?Sized>(&'a self, downstream: &'a M) -> ControlMemoryAccess<'a, M> {
        ControlMemoryAccess {
            control: self,
            downstream,
        }
    }
}

impl<M: ?Sized> ControlMemoryAccess<'_, M> {
    fn identity(&self, address: MemoryAddress) -> Result<MemoryIdentity, MemoryAccessError> {
        self.control
            .memory_identity()
            .map_err(|_| MemoryAccessError::Unmapped { address })
    }
}

impl<M: MemoryAccess<MemoryIdentity> + ?Sized> MemoryAccess for ControlMemoryAccess<'_, M> {
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.downstream
            .validate_with_sideband(&self.identity(address)?, address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.downstream
            .read_with_sideband(&self.identity(address)?, address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.downstream
            .write_with_sideband(&self.identity(address)?, address, alignment, bytes)
    }
}
