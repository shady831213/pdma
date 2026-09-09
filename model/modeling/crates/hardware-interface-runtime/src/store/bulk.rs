use super::backend::{MemoryState, inspect, mutate};
use super::{Memory, MemoryStorage, SharedMemory};
use crate::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion};

impl<P: MemoryStorage> MemoryAccess for Memory<P> {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        let access = CheckedAccess::extent(address, alignment, length)?;
        inspect(&self.backend, |state| {
            state.resolve(access.address, access.length).map(|_| ())
        })
    }

    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        let access = CheckedAccess::new(address, alignment, destination.len())?;
        inspect(&self.backend, |state| {
            let stored = state.resolve(access.address, access.length)?;
            let range = stored.range(access.address, access.end)?;
            let source = stored
                .bytes
                .get(range)
                .ok_or_else(|| access.crossing(stored.region))?;
            destination.copy_from_slice(source);
            Ok(())
        })
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        mutate(&self.backend, |state| {
            write_store(state, address, alignment, source)
        })
    }
}

impl MemoryAccess for SharedMemory {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.memory.validate(address, alignment, length)
    }

    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.read(address, alignment, destination)
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.write(address, alignment, source)
    }
}

pub(super) fn write_store(
    state: &mut MemoryState,
    address: MemoryAddress,
    alignment: MemoryAlignment,
    source: &[u8],
) -> Result<(), MemoryAccessError> {
    let access = CheckedAccess::new(address, alignment, source.len())?;
    let stored = state.resolve_mut(access.address, access.length)?;
    let region = stored.region;
    let range = stored.range(access.address, access.end)?;
    let destination = stored
        .bytes
        .get_mut(range)
        .ok_or_else(|| access.crossing(region))?;
    destination.copy_from_slice(source);
    Ok(())
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
        Self::extent(address, alignment, length)
    }

    fn extent(
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<Self, MemoryAccessError> {
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
