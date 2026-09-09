use std::fmt;
use std::sync::Arc;

use crate::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};

/// Cloneable address read/write capability with no storage-management authority.
#[derive(Clone)]
pub struct SharedMemoryAccess(Arc<dyn MemoryAccess + Send + Sync>);

impl SharedMemoryAccess {
    /// Reports whether both capabilities refer to the same injected backend.
    pub fn shares_backend(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: MemoryAccess + Send + Sync + 'static> From<Arc<T>> for SharedMemoryAccess {
    fn from(memory: Arc<T>) -> Self {
        Self(memory)
    }
}

impl fmt::Debug for SharedMemoryAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedMemoryAccess")
            .finish_non_exhaustive()
    }
}

impl MemoryAccess for SharedMemoryAccess {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.0.validate(address, alignment, length)
    }

    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.0.read(address, alignment, destination)
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.0.write(address, alignment, source)
    }
}
