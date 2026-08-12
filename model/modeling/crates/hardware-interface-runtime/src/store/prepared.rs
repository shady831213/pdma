use super::{SharedMemory, Store, StoredRegion, lock_store, validate_overlaps};
use crate::{MemoryError, MemoryRegion, MemoryRevision};

/// A preallocated public proposal bound to one observed revision.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedRegions {
    pub(super) expected_revision: MemoryRevision,
    pub(super) next_revision: MemoryRevision,
    pub(super) additions: Vec<StoredRegion>,
}

impl PreparedRegions {
    /// Consumes and installs this proposal if its observed revision is current.
    pub fn try_install(self, memory: &SharedMemory) -> Result<(), MemoryError> {
        let mut store = lock_store(&memory.inner);
        if store.revision != self.expected_revision {
            return Err(MemoryError::StaleRevision {
                expected_revision: self.expected_revision.raw(),
                actual_revision: store.revision.raw(),
            });
        }
        store.install(self);
        drop(store);
        Ok(())
    }

    pub(super) fn prepare(store: &Store, regions: &[MemoryRegion]) -> Result<Self, MemoryError> {
        validate_overlaps(store, regions)?;
        let next_revision = store
            .revision
            .checked_next()
            .ok_or(MemoryError::RevisionExhausted)?;
        let final_count = store.regions.len().checked_add(regions.len()).ok_or(
            MemoryError::RegionTableExhausted {
                entries: usize::MAX,
            },
        )?;
        let mut additions = Vec::new();
        additions.try_reserve_exact(final_count).map_err(|_| {
            MemoryError::RegionTableExhausted {
                entries: final_count,
            }
        })?;
        for region in regions {
            additions.push(StoredRegion::allocate(*region)?);
        }
        Ok(Self {
            expected_revision: store.revision,
            next_revision,
            additions,
        })
    }
}
