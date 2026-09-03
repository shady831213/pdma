use super::backend::{MemoryState, StoredRegion, mutate, validate_overlaps};
use super::{Memory, MemoryStorage, SharedMemory};
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
        self.try_install_on(&memory.memory)
    }

    /// Consumes and installs this proposal on generic checked memory.
    pub fn try_install_on<P: MemoryStorage>(self, memory: &Memory<P>) -> Result<(), MemoryError> {
        mutate(&memory.backend, |state| {
            if state.revision != self.expected_revision {
                return Err(MemoryError::StaleRevision {
                    expected_revision: self.expected_revision.raw(),
                    actual_revision: state.revision.raw(),
                });
            }
            state.install(self);
            Ok(())
        })
    }

    pub(super) fn prepare(
        state: &MemoryState,
        regions: &[MemoryRegion],
    ) -> Result<Self, MemoryError> {
        validate_overlaps(state, regions)?;
        let next_revision = state
            .revision
            .checked_next()
            .ok_or(MemoryError::RevisionExhausted)?;
        let final_count = state.regions.len().checked_add(regions.len()).ok_or(
            MemoryError::RegionTableExhausted {
                entries: usize::MAX,
            },
        )?;
        if state
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
        Ok(Self {
            expected_revision: state.revision,
            next_revision,
            additions,
        })
    }
}
