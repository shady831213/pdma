use std::fmt;

use crate::{MemoryError, MemoryRegion, MemoryRevision};

mod backend;
mod bulk;
mod prepared;
mod transaction;

pub use backend::{LocalMemoryStorage, MemoryStorage, SharedMemoryStorage};
pub use prepared::PreparedRegions;
pub use transaction::{MemoryTransaction, MemoryTransactionDraft, TransactionPreparedRegions};

use backend::{MemoryState, inspect, mutate};

/// Checked memory whose closure-scoped storage policy is selected by `P`.
///
/// Nested access through a clone retains the original synchronous blocking
/// behavior; [`crate::StorageError`] does not enter checked-memory signatures.
pub struct Memory<P: MemoryStorage> {
    pub(in crate::store) backend: P::Backend<MemoryState>,
}

impl<P: MemoryStorage> Memory<P> {
    /// Creates empty memory at revision zero with no policy capacity.
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

    /// Returns the current installed-region revision.
    pub fn revision(&self) -> MemoryRevision {
        inspect(&self.backend, |state| state.revision)
    }

    /// Returns installed metadata in ascending address order.
    pub fn regions(&self) -> Vec<MemoryRegion> {
        inspect(&self.backend, |state| {
            state.regions.iter().map(|stored| stored.region).collect()
        })
    }

    /// Preallocates a revision-bound, mutation-free region proposal.
    pub fn prepare_regions(
        &self,
        regions: &[MemoryRegion],
    ) -> Result<PreparedRegions, MemoryError> {
        inspect(&self.backend, |state| {
            PreparedRegions::prepare(state, regions)
        })
    }

    /// Runs a synchronous operation with one exclusive memory view.
    ///
    /// References cannot escape the generic local transaction:
    ///
    /// ```compile_fail
    /// use hardware_interface_runtime::{LocalMemoryStorage, Memory};
    /// let memory = Memory::<LocalMemoryStorage>::new();
    /// let escaped = memory.transaction(|transaction| transaction);
    /// drop(escaped);
    /// ```
    ///
    /// Futures borrowing a transaction cannot escape either:
    ///
    /// ```compile_fail
    /// use hardware_interface_runtime::{LocalMemoryStorage, Memory};
    /// let memory = Memory::<LocalMemoryStorage>::new();
    /// let escaped = memory.transaction(|transaction| async move { drop(transaction) });
    /// drop(escaped);
    /// ```
    pub fn transaction<R>(
        &self,
        operation: impl for<'state> FnOnce(&mut MemoryTransaction<'state>) -> R,
    ) -> R {
        mutate(&self.backend, |state| {
            let mut transaction = MemoryTransaction::new(state);
            operation(&mut transaction)
        })
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
    /// Creates an empty store at revision zero with no policy capacity.
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

    /// Returns the current installed-region revision.
    pub fn revision(&self) -> MemoryRevision {
        self.memory.revision()
    }

    /// Returns installed metadata in ascending address order.
    pub fn regions(&self) -> Vec<MemoryRegion> {
        self.memory.regions()
    }

    /// Preallocates a revision-bound, mutation-free region proposal.
    pub fn prepare_regions(
        &self,
        regions: &[MemoryRegion],
    ) -> Result<PreparedRegions, MemoryError> {
        self.memory.prepare_regions(regions)
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
        operation: impl for<'state> FnOnce(&mut MemoryTransaction<'state>) -> R,
    ) -> R {
        self.memory.transaction(operation)
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
