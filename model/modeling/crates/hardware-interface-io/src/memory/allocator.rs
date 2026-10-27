use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::{Arc, Mutex};

use crate::memory::{
    MemoryAccessError, MemoryError, MemoryRegion, MemoryRegionError, SharedMemory,
};
use crate::memory::{MemoryAddress, MemoryAlignment, SharedMemoryAccess};

/// Failure to reserve an address and install its backing region.
#[derive(Debug, PartialEq, Eq)]
pub enum MemoryAllocationError {
    /// Alignment or cursor advancement exceeds the address space.
    AddressExhausted,
    /// The requested backing region cannot be represented.
    Region(MemoryRegionError),
    /// Backing installation failed before cursor advancement.
    Storage(MemoryError),
}

impl Display for MemoryAllocationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressExhausted => {
                formatter.write_str("memory allocation address space is exhausted")
            }
            Self::Region(error) => Display::fmt(error, formatter),
            Self::Storage(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for MemoryAllocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AddressExhausted => None,
            Self::Region(error) => Some(error),
            Self::Storage(error) => Some(error),
        }
    }
}

/// Clone-shared monotonic address allocation over one shared byte store.
///
/// Clones serialize cursor advancement and backing installation. Separately constructed
/// allocators have independent cursors; installed-region overlap remains a typed error.
#[derive(Clone, Debug)]
pub struct SharedMemoryAllocator {
    memory: Arc<SharedMemory>,
    cursor: Arc<Mutex<MemoryAddress>>,
}

impl SharedMemoryAllocator {
    /// Creates an allocator starting at the caller-selected address.
    pub fn new(memory: Arc<SharedMemory>, first_address: MemoryAddress) -> Self {
        Self {
            memory,
            cursor: Arc::new(Mutex::new(first_address)),
        }
    }

    /// Returns the backing store's capability without allocation authority.
    pub fn memory_access(&self) -> SharedMemoryAccess<(), MemoryAccessError> {
        SharedMemoryAccess::from(Arc::clone(&self.memory))
    }

    /// Reserves an aligned address and installs zero-initialized backing for nonzero length.
    ///
    /// Zero length advances the cursor by one byte without installing a region.
    /// Any returned error leaves this allocator's cursor and backing unchanged.
    pub fn allocate(
        &self,
        length: u64,
        alignment: MemoryAlignment,
    ) -> Result<MemoryAddress, MemoryAllocationError> {
        let mut cursor = self
            .cursor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mask = alignment.raw() - 1;
        let aligned = cursor
            .raw()
            .checked_add(mask)
            .ok_or(MemoryAllocationError::AddressExhausted)?
            & !mask;
        let address = MemoryAddress::new(aligned);
        let next = address
            .checked_add(length.max(1))
            .ok_or(MemoryAllocationError::AddressExhausted)?;
        if length > 0 {
            let region = MemoryRegion::try_new(address, length, alignment)
                .map_err(MemoryAllocationError::Region)?;
            self.memory
                .add_regions(&[region])
                .map_err(MemoryAllocationError::Storage)?;
        }
        *cursor = next;
        drop(cursor);
        Ok(address)
    }
}
