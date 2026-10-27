use std::ops::Range;

use crate::{
    LocalStorage, MemoryAccessError, MemoryAddress, MemoryError, MemoryRegion, MutexStorage,
    StorageBackend,
};

/// Selects closure-scoped storage for a generic checked memory.
pub trait MemoryStorage {
    /// Storage instantiated with the private memory state.
    type Backend<T>: StorageBackend<T> + Clone;

    /// Creates storage containing `value`.
    fn create<T>(value: T) -> Self::Backend<T>;
}

/// Single-thread storage policy for [`super::Memory`].
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalMemoryStorage;

impl MemoryStorage for LocalMemoryStorage {
    type Backend<T> = LocalStorage<T>;

    fn create<T>(value: T) -> Self::Backend<T> {
        LocalStorage::new(value)
    }
}

/// Thread-safe storage policy used by [`super::SharedMemory`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SharedMemoryStorage;

impl MemoryStorage for SharedMemoryStorage {
    type Backend<T> = MutexStorage<T>;

    fn create<T>(value: T) -> Self::Backend<T> {
        MutexStorage::new(value)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct MemoryState {
    pub(super) regions: Vec<StoredRegion>,
    pub(super) region_capacity: Option<usize>,
}

impl MemoryState {
    pub(super) const fn new() -> Self {
        Self {
            regions: Vec::new(),
            region_capacity: None,
        }
    }

    pub(super) const fn with_region_capacity(capacity: usize) -> Self {
        Self {
            regions: Vec::new(),
            region_capacity: Some(capacity),
        }
    }

    pub(super) fn add_regions(&mut self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        validate_overlaps(self, regions)?;
        let final_count = self.regions.len().checked_add(regions.len()).ok_or(
            MemoryError::RegionTableExhausted {
                entries: usize::MAX,
            },
        )?;
        if self
            .region_capacity
            .is_some_and(|capacity| final_count > capacity)
        {
            return Err(MemoryError::RegionTableExhausted {
                entries: final_count,
            });
        }
        let mut additions = Vec::new();
        additions.try_reserve_exact(final_count).map_err(|_| {
            MemoryError::RegionTableExhausted {
                entries: final_count,
            }
        })?;
        for region in regions {
            additions.push(StoredRegion::allocate(*region)?);
        }
        additions.append(&mut self.regions);
        additions.sort_unstable_by_key(|stored| stored.region.base());
        self.regions = additions;
        Ok(())
    }

    pub(super) fn resolve(
        &self,
        address: MemoryAddress,
        length: u64,
    ) -> Result<&StoredRegion, MemoryAccessError> {
        let index = resolve_index(&self.regions, address, length)?;
        self.regions
            .get(index)
            .ok_or(MemoryAccessError::Unmapped { address })
    }

    pub(super) fn resolve_mut(
        &mut self,
        address: MemoryAddress,
        length: u64,
    ) -> Result<&mut StoredRegion, MemoryAccessError> {
        let index = resolve_index(&self.regions, address, length)?;
        self.regions
            .get_mut(index)
            .ok_or(MemoryAccessError::Unmapped { address })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct StoredRegion {
    pub(super) region: MemoryRegion,
    pub(super) bytes: Vec<u8>,
}

impl StoredRegion {
    pub(super) fn allocate(region: MemoryRegion) -> Result<Self, MemoryError> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(region.storage_length())
            .map_err(|_| MemoryError::StorageExhausted {
                length: region.length(),
            })?;
        bytes.resize(region.storage_length(), 0);
        Ok(Self { region, bytes })
    }

    pub(super) fn range(
        &self,
        address: MemoryAddress,
        end: MemoryAddress,
    ) -> Result<Range<usize>, MemoryAccessError> {
        let length = end.raw() - address.raw();
        let start = usize::try_from(address.raw() - self.region.base().raw()).map_err(|_| {
            MemoryAccessError::CrossesRegion {
                address,
                length,
                region: self.region,
            }
        })?;
        let finish = usize::try_from(end.raw() - self.region.base().raw()).map_err(|_| {
            MemoryAccessError::CrossesRegion {
                address,
                length,
                region: self.region,
            }
        })?;
        Ok(start..finish)
    }
}

pub(super) fn validate_overlaps(
    state: &MemoryState,
    proposed: &[MemoryRegion],
) -> Result<(), MemoryError> {
    for (index, second) in proposed.iter().enumerate() {
        for first in proposed.iter().take(index) {
            if first.overlaps(*second) {
                return Err(MemoryError::Overlap {
                    first: *first,
                    second: *second,
                });
            }
        }
    }
    for second in proposed {
        for first in &state.regions {
            if first.region.overlaps(*second) {
                return Err(MemoryError::Overlap {
                    first: first.region,
                    second: *second,
                });
            }
        }
    }
    Ok(())
}

fn resolve_index(
    regions: &[StoredRegion],
    address: MemoryAddress,
    length: u64,
) -> Result<usize, MemoryAccessError> {
    let index = regions.iter().position(|stored| {
        let base = stored.region.base().raw();
        let end = stored.region.end().raw();
        if length == 0 {
            base <= address.raw() && address.raw() <= end
        } else {
            base <= address.raw() && address.raw() < end
        }
    });
    let index = index.ok_or(MemoryAccessError::Unmapped { address })?;
    let region = regions
        .get(index)
        .ok_or(MemoryAccessError::Unmapped { address })?
        .region;
    let end = address
        .checked_add(length)
        .ok_or(MemoryAccessError::AddressOverflow { address, length })?;
    if end.raw() > region.end().raw() {
        return Err(MemoryAccessError::CrossesRegion {
            address,
            length,
            region,
        });
    }
    Ok(index)
}
