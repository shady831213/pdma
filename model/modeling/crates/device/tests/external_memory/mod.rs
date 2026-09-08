use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::{BufferId, ResourceConfig};
use pdma_model_device::{AsyncDevice, MappedDmaRegion, PdmaMemoryProvisioning};
use std::sync::{Arc, Mutex};

mod exact_abi;
mod failures;

struct ExternalBytes<const N: usize = 128>(Mutex<[u8; N]>);

impl<const N: usize> MemoryAccess for ExternalBytes<N> {
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        if address.raw() % alignment.raw() != 0 {
            return Err(MemoryAccessError::Misaligned { address, alignment });
        }
        let offset = usize::try_from(
            address
                .raw()
                .checked_sub(0x8000)
                .ok_or(MemoryAccessError::Unmapped { address })?,
        )
        .map_err(|_| MemoryAccessError::Unmapped { address })?;
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source = bytes
            .get(offset..offset.saturating_add(destination.len()))
            .ok_or(MemoryAccessError::Unmapped { address })?;
        destination.copy_from_slice(source);
        drop(bytes);
        Ok(())
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        if address.raw() % alignment.raw() != 0 {
            return Err(MemoryAccessError::Misaligned { address, alignment });
        }
        let offset = usize::try_from(
            address
                .raw()
                .checked_sub(0x8000)
                .ok_or(MemoryAccessError::Unmapped { address })?,
        )
        .map_err(|_| MemoryAccessError::Unmapped { address })?;
        let mut bytes = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let destination = bytes
            .get_mut(offset..offset.saturating_add(source.len()))
            .ok_or(MemoryAccessError::Unmapped { address })?;
        destination.copy_from_slice(source);
        drop(bytes);
        Ok(())
    }
}
