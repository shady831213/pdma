use std::fmt;
use std::sync::Arc;

use crate::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};

/// Cloneable address read/write capability with no storage-management authority.
pub struct SharedMemoryAccess<S = ()>(Arc<dyn MemoryAccess<S> + Send + Sync>);

impl<S> SharedMemoryAccess<S> {
    /// Reports whether both capabilities refer to the same injected backend.
    pub fn shares_backend(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<S> Clone for SharedMemoryAccess<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<S, T: MemoryAccess<S> + Send + Sync + 'static> From<Arc<T>> for SharedMemoryAccess<S> {
    fn from(memory: Arc<T>) -> Self {
        Self(memory)
    }
}

impl<S> fmt::Debug for SharedMemoryAccess<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedMemoryAccess")
            .finish_non_exhaustive()
    }
}

impl<S> MemoryAccess<S> for SharedMemoryAccess<S> {
    fn validate_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.0
            .validate_with_sideband(sideband, address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.0
            .read_with_sideband(sideband, address, alignment, destination)
    }

    fn write_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.0
            .write_with_sideband(sideband, address, alignment, source)
    }
}
