use super::backend::MemoryState;
use super::{Memory, MemoryStorage};
use crate::{AtomicAccessError, AtomicOperation, StorageBackend};
use crate::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion};

impl<P: MemoryStorage> MemoryAccess for Memory<P> {
    type Error = MemoryAccessError;

    fn atomic32_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        let access = CheckedAccess::new(address, MemoryAlignment::WORD, 4)
            .map_err(AtomicAccessError::Access)?;
        self.backend
            .mutate(|state| {
                atomic_store(state, access, |bytes| {
                    operation
                        .replacement(u32::from_le_bytes(bytes))
                        .map(u32::to_le_bytes)
                })
                .map(u32::from_le_bytes)
            })
            .map_err(MemoryAccessError::Storage)
            .and_then(std::convert::identity)
            .map_err(AtomicAccessError::Access)
    }

    fn atomic64_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        let access = CheckedAccess::new(address, MemoryAlignment::DOUBLE_WORD, 8)
            .map_err(AtomicAccessError::Access)?;
        self.backend
            .mutate(|state| {
                atomic_store(state, access, |bytes| {
                    operation
                        .replacement(u64::from_le_bytes(bytes))
                        .map(u64::to_le_bytes)
                })
                .map(u64::from_le_bytes)
            })
            .map_err(MemoryAccessError::Storage)
            .and_then(std::convert::identity)
            .map_err(AtomicAccessError::Access)
    }

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        let access = CheckedAccess::extent(address, alignment, length)?;
        self.backend
            .inspect(|state| state.resolve(access.address, access.length).map(|_| ()))
            .map_err(MemoryAccessError::Storage)?
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        let access = CheckedAccess::new(address, alignment, destination.len())?;
        self.backend
            .inspect(|state| {
                let stored = state.resolve(access.address, access.length)?;
                let range = stored.range(access.address, access.end)?;
                let source = stored
                    .bytes
                    .get(range)
                    .ok_or_else(|| access.crossing(stored.region))?;
                destination.copy_from_slice(source);
                Ok(())
            })
            .map_err(MemoryAccessError::Storage)?
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.backend
            .mutate(|state| write_store(state, address, alignment, source))
            .map_err(MemoryAccessError::Storage)?
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

fn atomic_store<const N: usize>(
    state: &mut MemoryState,
    access: CheckedAccess,
    replacement: impl FnOnce([u8; N]) -> Option<[u8; N]>,
) -> Result<[u8; N], MemoryAccessError> {
    let stored = state.resolve_mut(access.address, access.length)?;
    let region = stored.region;
    let range = stored.range(access.address, access.end)?;
    let destination = stored
        .bytes
        .get_mut(range)
        .ok_or_else(|| access.crossing(region))?;
    let mut old = [0; N];
    old.copy_from_slice(destination);
    if let Some(new) = replacement(old) {
        destination.copy_from_slice(&new);
    }
    Ok(old)
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
