#![allow(
    clippy::expect_used,
    reason = "fixed external test extents and poisoned fixture locks are test failures"
)]

use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion,
};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::AsyncDevice;
use std::sync::{Arc, Mutex};

mod address_only;
mod exact_abi;
mod failures;

struct ExternalBytes<const N: usize = 128>(Mutex<[u8; N]>);

impl<const N: usize> MemoryAccess for ExternalBytes<N> {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        let end = address
            .checked_add(length)
            .ok_or(MemoryAccessError::AddressOverflow { address, length })?;
        if address.raw() % alignment.raw() != 0 {
            return Err(MemoryAccessError::Misaligned { address, alignment });
        }
        let region = MemoryRegion::try_new(
            MemoryAddress::new(0x8000),
            u64::try_from(N).expect("external extent fits the address space"),
            MemoryAlignment::ONE,
        )
        .expect("external extent is nonempty and bounded");
        let extent_end = region
            .base()
            .checked_add(region.length())
            .expect("bounded external extent");
        if address < region.base() || address > extent_end || (address == extent_end && length != 0)
        {
            return Err(MemoryAccessError::Unmapped { address });
        }
        if end > extent_end {
            return Err(MemoryAccessError::CrossesRegion {
                address,
                length,
                region,
            });
        }
        Ok(())
    }

    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.validate(
            address,
            alignment,
            u64::try_from(destination.len()).expect("slice length"),
        )?;
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
        self.validate(
            address,
            alignment,
            u64::try_from(source.len()).expect("slice length"),
        )?;
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
