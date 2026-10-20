use std::fmt;

use crate::{MemoryError, MemoryRegion, StorageBackend, StorageError};

mod backend;
mod bulk;
#[cfg(test)]
mod tests;

pub use backend::{LocalMemoryStorage, MemoryStorage, SharedMemoryStorage};

use backend::MemoryState;

/// Checked memory whose closure-scoped storage policy is selected by `P`.
pub struct Memory<P: MemoryStorage> {
    pub(in crate::store) backend: P::Backend<MemoryState>,
}

impl<P: MemoryStorage> Memory<P> {
    /// Creates empty memory with no policy capacity.
    pub fn new() -> Self {
        Self {
            backend: P::create(MemoryState::new()),
        }
    }

    /// Creates empty memory with an explicit installed-region capacity.
    pub fn with_region_capacity(capacity: usize) -> Self {
        Self {
            backend: P::create(MemoryState::with_region_capacity(capacity)),
        }
    }

    /// Returns installed metadata in ascending address order.
    ///
    /// # Errors
    /// Returns a storage error if called reentrantly on the same backend.
    pub fn regions(&self) -> Result<Vec<MemoryRegion>, StorageError> {
        self.backend
            .inspect(|state| state.regions.iter().map(|stored| stored.region).collect())
    }

    /// Atomically installs a batch of zero-initialized regions.
    ///
    /// Empty batches are no-ops. All validation and allocation finish before
    /// installed regions change; any error preserves existing regions and bytes.
    ///
    /// # Errors
    /// Returns a storage, overlap, region-table capacity, or backing allocation error.
    pub fn add_regions(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        if regions.is_empty() {
            return Ok(());
        }
        self.backend
            .mutate(|state| state.add_regions(regions))
            .map_err(MemoryError::Storage)?
    }
}

impl<P: MemoryStorage> Clone for Memory<P> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
        }
    }
}

impl<P: MemoryStorage> fmt::Debug for Memory<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Memory").finish_non_exhaustive()
    }
}

impl<P: MemoryStorage> Default for Memory<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// Clone-shared storage for checked memory regions and bytes.
pub type SharedMemory = Memory<SharedMemoryStorage>;
