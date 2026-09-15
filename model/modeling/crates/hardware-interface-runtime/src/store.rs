use std::fmt;

use crate::{MemoryError, MemoryRegion};

mod backend;
mod bulk;

pub use backend::{LocalMemoryStorage, MemoryStorage, SharedMemoryStorage};

use backend::{MemoryState, inspect, mutate};

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
    pub fn regions(&self) -> Vec<MemoryRegion> {
        inspect(&self.backend, |state| {
            state.regions.iter().map(|stored| stored.region).collect()
        })
    }

    /// Atomically installs a batch of zero-initialized regions.
    ///
    /// Empty batches are no-ops. All validation and allocation finish before
    /// installed regions change; any error preserves existing regions and bytes.
    ///
    /// # Errors
    /// Returns an overlap, region-table capacity, or backing allocation error.
    pub fn add_regions(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        if regions.is_empty() {
            return Ok(());
        }
        mutate(&self.backend, |state| state.add_regions(regions))
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
#[derive(Clone)]
pub struct SharedMemory {
    pub(in crate::store) memory: Memory<SharedMemoryStorage>,
}

impl SharedMemory {
    /// Creates an empty store with no policy capacity.
    pub fn new() -> Self {
        Self {
            memory: Memory::new(),
        }
    }

    /// Creates an empty store with an explicit installed-region capacity.
    pub fn with_region_capacity(capacity: usize) -> Self {
        Self {
            memory: Memory::with_region_capacity(capacity),
        }
    }

    /// Returns installed metadata in ascending address order.
    pub fn regions(&self) -> Vec<MemoryRegion> {
        self.memory.regions()
    }

    /// Atomically installs zero-initialized regions; errors preserve the store.
    ///
    /// # Errors
    /// Returns an overlap, region-table capacity, or backing allocation error.
    pub fn add_regions(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        self.memory.add_regions(regions)
    }
}

impl fmt::Debug for SharedMemory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedMemory")
            .finish_non_exhaustive()
    }
}

impl Default for SharedMemory {
    fn default() -> Self {
        Self::new()
    }
}
