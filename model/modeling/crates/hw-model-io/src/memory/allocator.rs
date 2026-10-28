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
    /// The heap's base plus size exceeds the address space.
    InvalidRange,
    /// Alignment or cursor advancement exceeds the configured heap.
    AddressExhausted,
    /// The requested backing region cannot be represented.
    Region(MemoryRegionError),
    /// Backing installation failed before cursor advancement.
    Storage(MemoryError),
}

impl Display for MemoryAllocationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange => formatter.write_str("memory allocation range overflows"),
            Self::AddressExhausted => formatter.write_str("memory allocation heap is exhausted"),
            Self::Region(error) => Display::fmt(error, formatter),
            Self::Storage(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for MemoryAllocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRange | Self::AddressExhausted => None,
            Self::Region(error) => Some(error),
            Self::Storage(error) => Some(error),
        }
    }
}

/// Clone-shared bounded monotonic allocation over one shared byte store.
///
/// Clones serialize cursor advancement and backing installation. Separately constructed
/// allocators have independent cursors; installed-region overlap remains a typed error.
#[derive(Clone, Debug)]
pub struct SharedMemoryAllocator {
    memory: Arc<SharedMemory>,
    cursor: Arc<Mutex<MemoryAddress>>,
    end: MemoryAddress,
}

impl SharedMemoryAllocator {
    /// Creates an allocator for `[base, base + size)` without installing backing.
    ///
    /// Empty heaps are valid but cannot satisfy any allocation, even zero length.
    ///
    /// # Errors
    /// Returns [`MemoryAllocationError::InvalidRange`] if `base + size` overflows.
    pub fn new(
        memory: Arc<SharedMemory>,
        base: MemoryAddress,
        size: u64,
    ) -> Result<Self, MemoryAllocationError> {
        let end = base
            .checked_add(size)
            .ok_or(MemoryAllocationError::InvalidRange)?;
        Ok(Self {
            memory,
            cursor: Arc::new(Mutex::new(base)),
            end,
        })
    }

    /// Returns the backing store's capability without allocation authority.
    pub fn memory_access(&self) -> SharedMemoryAccess<(), MemoryAccessError> {
        SharedMemoryAccess::from(Arc::clone(&self.memory))
    }

    /// Reserves an aligned address and installs zero-initialized backing for nonzero length.
    ///
    /// Zero length advances the cursor by one byte without installing a region.
    /// Alignment padding and that byte both consume the configured heap.
    /// Any returned error leaves this allocator's cursor and backing unchanged.
    ///
    /// # Errors
    /// Returns [`MemoryAllocationError::AddressExhausted`] if the reservation cannot
    /// fit, or the original region or storage error if backing installation fails.
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
        if next > self.end {
            return Err(MemoryAllocationError::AddressExhausted);
        }
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
