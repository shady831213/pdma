use std::fmt;
use std::sync::Arc;

use crate::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};

/// Cloneable address read/write capability with no storage-management authority.
pub struct SharedMemoryAccess<S = (), E = MemoryAccessError>(
    Arc<dyn MemoryAccess<S, Error = E> + Send + Sync>,
);

impl<S, E> SharedMemoryAccess<S, E> {
    /// Reports whether both capabilities refer to the same injected backend.
    pub fn shares_backend(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<S, E> Clone for SharedMemoryAccess<S, E> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<S, E, T: MemoryAccess<S, Error = E> + Send + Sync + 'static> From<Arc<T>>
    for SharedMemoryAccess<S, E>
{
    fn from(memory: Arc<T>) -> Self {
        Self(memory)
    }
}

impl<S, E> fmt::Debug for SharedMemoryAccess<S, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedMemoryAccess")
            .finish_non_exhaustive()
    }
}

impl<S, E> MemoryAccess<S> for SharedMemoryAccess<S, E> {
    type Error = E;

    fn validate_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.0
            .validate_with_sideband(sideband, address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.0
            .read_with_sideband(sideband, address, alignment, destination)
    }

    fn write_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        self.0
            .write_with_sideband(sideband, address, alignment, source)
    }
}
