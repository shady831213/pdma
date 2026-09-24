use super::ContextTable;
use crate::windows::ChannelWindows;
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::DmaAccess;
use pdma_model_contract::memory::{FunctionId, MemoryIdentity};

#[doc = "Borrowed context lookup translating DMA selectors into authorized fabric identities."]
#[derive(Debug)]
pub struct ContextMemoryAccess<'a, M: ?Sized> {
    table: &'a ContextTable,
    windows: &'a ChannelWindows,
    downstream: &'a M,
}

impl ContextTable {
    #[doc = "Borrows the actual owner registers and platform fabric without owning memory backing."]
    pub const fn memory<'a, M: ?Sized>(
        &'a self,
        windows: &'a ChannelWindows,
        downstream: &'a M,
    ) -> ContextMemoryAccess<'a, M> {
        ContextMemoryAccess {
            table: self,
            windows,
            downstream,
        }
    }
}

impl<M: ?Sized> ContextMemoryAccess<'_, M> {
    fn identity(
        &self,
        access: DmaAccess,
        address: MemoryAddress,
    ) -> Result<MemoryIdentity, MemoryAccessError> {
        let unmapped = MemoryAccessError::Unmapped { address };
        let owner = self.windows.owner(access.channel).ok_or(unmapped)?;
        let entry = usize::try_from(access.context.raw())
            .ok()
            .and_then(|index| self.table.entries.get(index))
            .filter(|entry| entry.values.valid.enabled())
            .ok_or(unmapped)?;
        if FunctionId::new(entry.values.owner.raw()) != owner {
            return Err(unmapped);
        }
        Ok(MemoryIdentity {
            function: owner,
            attributes: entry.values.attributes.attributes().map_err(|_| unmapped)?,
        })
    }
}

impl<M: MemoryAccess<MemoryIdentity> + ?Sized> MemoryAccess<DmaAccess>
    for ContextMemoryAccess<'_, M>
{
    fn validate_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.downstream.validate_with_sideband(
            &self.identity(*access, address)?,
            address,
            alignment,
            length,
        )
    }
    fn read_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.downstream.read_with_sideband(
            &self.identity(*access, address)?,
            address,
            alignment,
            bytes,
        )
    }
    fn write_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.downstream.write_with_sideband(
            &self.identity(*access, address)?,
            address,
            alignment,
            bytes,
        )
    }
}
