use super::{SharedMemory, lock_store};
use crate::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion};

impl MemoryAccess for SharedMemory {
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        let access = CheckedAccess::new(address, alignment, destination.len())?;
        let store = lock_store(&self.inner);
        let stored = store.resolve(access.address, access.length)?;
        let range = stored.range(access.address, access.end)?;
        let source = stored
            .bytes
            .get(range)
            .ok_or_else(|| access.crossing(stored.region))?;
        destination.copy_from_slice(source);
        drop(store);
        Ok(())
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        let access = CheckedAccess::new(address, alignment, source.len())?;
        let mut store = lock_store(&self.inner);
        let stored = store.resolve_mut(access.address, access.length)?;
        let region = stored.region;
        let range = stored.range(access.address, access.end)?;
        let destination = stored
            .bytes
            .get_mut(range)
            .ok_or_else(|| access.crossing(region))?;
        destination.copy_from_slice(source);
        drop(store);
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct CheckedAccess {
    address: MemoryAddress,
    end: MemoryAddress,
    length: u64,
}

impl CheckedAccess {
    fn new(
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: usize,
    ) -> Result<Self, MemoryAccessError> {
        let length = u64::try_from(length).map_err(|_| MemoryAccessError::AddressOverflow {
            address,
            length: u64::MAX,
        })?;
        let end = address
            .checked_add(length)
            .ok_or(MemoryAccessError::AddressOverflow { address, length })?;
        if !alignment.accepts(address) {
            return Err(MemoryAccessError::Misaligned { address, alignment });
        }
        Ok(Self {
            address,
            end,
            length,
        })
    }

    const fn crossing(self, region: MemoryRegion) -> MemoryAccessError {
        MemoryAccessError::CrossesRegion {
            address: self.address,
            length: self.length,
            region,
        }
    }
}
