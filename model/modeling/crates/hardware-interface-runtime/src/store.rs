use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{MemoryAccessError, MemoryAddress, MemoryError, MemoryRegion, MemoryRevision};

mod bulk;
mod prepared;
mod transaction;

pub use prepared::PreparedRegions;
pub use transaction::{MemoryTransaction, MemoryTransactionDraft, TransactionPreparedRegions};

/// Clone-shared storage for checked memory regions and bytes.
#[derive(Clone, Debug)]
pub struct SharedMemory {
    inner: Arc<Mutex<Store>>,
}

impl SharedMemory {
    /// Creates an empty store at revision zero with no policy capacity.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Store::new())),
        }
    }

    /// Creates an empty store with an explicit installed-region capacity.
    pub fn with_region_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Store::with_region_capacity(capacity))),
        }
    }

    /// Returns the current installed-region revision.
    pub fn revision(&self) -> MemoryRevision {
        lock_store(&self.inner).revision
    }

    /// Returns installed metadata in ascending address order.
    pub fn regions(&self) -> Vec<MemoryRegion> {
        lock_store(&self.inner)
            .regions
            .iter()
            .map(|stored| stored.region)
            .collect()
    }

    /// Preallocates a revision-bound, mutation-free region proposal.
    pub fn prepare_regions(
        &self,
        regions: &[MemoryRegion],
    ) -> Result<PreparedRegions, MemoryError> {
        PreparedRegions::prepare(&lock_store(&self.inner), regions)
    }

    /// Runs a synchronous operation with one exclusive memory view.
    ///
    /// ```compile_fail
    /// use hardware_interface_runtime::SharedMemory;
    /// let memory = SharedMemory::new();
    /// let escaped = memory.transaction(|transaction| transaction);
    /// std::thread::spawn(move || drop(escaped));
    /// ```
    ///
    /// ```compile_fail
    /// use hardware_interface_runtime::SharedMemory;
    /// let memory = SharedMemory::new();
    /// memory.transaction(|transaction| {
    ///     std::thread::scope(|scope| {
    ///         scope.spawn(move || drop(transaction));
    ///     });
    /// });
    /// ```
    pub fn transaction<R>(
        &self,
        operation: impl for<'store> FnOnce(&mut MemoryTransaction<'store>) -> R,
    ) -> R {
        let mut store = lock_store(&self.inner);
        let result = {
            let mut transaction = MemoryTransaction::new(&mut store);
            operation(&mut transaction)
        };
        drop(store);
        result
    }

    /// Creates an independent fallible snapshot of regions and bytes.
    pub fn try_snapshot(&self) -> Result<Self, MemoryError> {
        let source = lock_store(&self.inner);
        Ok(Self {
            inner: Arc::new(Mutex::new(source.try_clone()?)),
        })
    }

    /// Replaces this store from an independently prepared snapshot.
    pub fn install_snapshot(&self, snapshot: &Self) {
        let mut source = lock_store(&snapshot.inner);
        let prepared = std::mem::replace(&mut *source, Store::new());
        drop(source);
        *lock_store(&self.inner) = prepared;
    }
}

impl Default for SharedMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Store {
    revision: MemoryRevision,
    regions: Vec<StoredRegion>,
    region_capacity: Option<usize>,
}

impl Store {
    const fn new() -> Self {
        Self {
            revision: MemoryRevision::INITIAL,
            regions: Vec::new(),
            region_capacity: None,
        }
    }

    const fn with_region_capacity(capacity: usize) -> Self {
        Self {
            revision: MemoryRevision::INITIAL,
            regions: Vec::new(),
            region_capacity: Some(capacity),
        }
    }

    fn install(&mut self, mut prepared: PreparedRegions) {
        let existing = std::mem::take(&mut self.regions);
        prepared.additions.extend(existing);
        prepared
            .additions
            .sort_unstable_by_key(|stored| stored.region.base());
        self.regions = prepared.additions;
        self.revision = prepared.next_revision;
    }

    fn try_clone(&self) -> Result<Self, MemoryError> {
        let mut regions = Vec::new();
        regions.try_reserve_exact(self.regions.len()).map_err(|_| {
            MemoryError::RegionTableExhausted {
                entries: self.regions.len(),
            }
        })?;
        for stored in &self.regions {
            regions.push(stored.try_clone()?);
        }
        Ok(Self {
            revision: self.revision,
            regions,
            region_capacity: self.region_capacity,
        })
    }

    pub(crate) fn resolve(
        &self,
        address: MemoryAddress,
        length: u64,
    ) -> Result<&StoredRegion, MemoryAccessError> {
        let index = resolve_index(&self.regions, address, length)?;
        self.regions
            .get(index)
            .ok_or(MemoryAccessError::Unmapped { address })
    }

    pub(crate) fn resolve_mut(
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
struct StoredRegion {
    region: MemoryRegion,
    bytes: Vec<u8>,
}

impl StoredRegion {
    fn allocate(region: MemoryRegion) -> Result<Self, MemoryError> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(region.storage_length())
            .map_err(|_| MemoryError::StorageExhausted {
                length: region.length(),
            })?;
        bytes.resize(region.storage_length(), 0);
        Ok(Self { region, bytes })
    }

    fn try_clone(&self) -> Result<Self, MemoryError> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.bytes.len())
            .map_err(|_| MemoryError::StorageExhausted {
                length: self.region.length(),
            })?;
        bytes.extend_from_slice(&self.bytes);
        Ok(Self {
            region: self.region,
            bytes,
        })
    }

    pub(crate) fn range(
        &self,
        address: MemoryAddress,
        end: MemoryAddress,
    ) -> Result<Range<usize>, MemoryAccessError> {
        let start = address.raw() - self.region.base().raw();
        let finish = end.raw() - self.region.base().raw();
        let start = usize::try_from(start).map_err(|_| MemoryAccessError::CrossesRegion {
            address,
            length: end.raw() - address.raw(),
            region: self.region,
        })?;
        let finish = usize::try_from(finish).map_err(|_| MemoryAccessError::CrossesRegion {
            address,
            length: end.raw() - address.raw(),
            region: self.region,
        })?;
        Ok(start..finish)
    }
}

fn lock_store(inner: &Arc<Mutex<Store>>) -> MutexGuard<'_, Store> {
    inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn validate_overlaps(store: &Store, proposed: &[MemoryRegion]) -> Result<(), MemoryError> {
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
        for first in &store.regions {
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
