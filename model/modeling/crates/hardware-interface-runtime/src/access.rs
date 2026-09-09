use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{MemoryAddress, MemoryAlignment, MemoryRegion};

/// Failure of one checked contiguous memory access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryAccessError {
    /// The exclusive access end exceeds the address space.
    AddressOverflow {
        /// Access start address.
        address: MemoryAddress,
        /// Requested byte length.
        length: u64,
    },
    /// The start address does not meet the caller's alignment requirement.
    Misaligned {
        /// Access start address.
        address: MemoryAddress,
        /// Required alignment.
        alignment: MemoryAlignment,
    },
    /// No installed region contains the access start or zero-length boundary.
    Unmapped {
        /// Access start address.
        address: MemoryAddress,
    },
    /// The access begins in one region but ends outside it.
    CrossesRegion {
        /// Access start address.
        address: MemoryAddress,
        /// Requested byte length.
        length: u64,
        /// Region containing the access start.
        region: MemoryRegion,
    },
}

impl Display for MemoryAccessError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressOverflow { address, length } => write!(
                formatter,
                "memory access at {:#x} with length {length} exceeds the address space",
                address.raw()
            ),
            Self::Misaligned { address, alignment } => write!(
                formatter,
                "memory access at {:#x} is not aligned to {}",
                address.raw(),
                alignment.raw()
            ),
            Self::Unmapped { address } => {
                write!(formatter, "memory address {:#x} is unmapped", address.raw())
            }
            Self::CrossesRegion {
                address,
                length,
                region,
            } => write!(
                formatter,
                "memory access at {:#x} with length {length} crosses region {:#x}..{:#x}",
                address.raw(),
                region.base().raw(),
                region.end().raw()
            ),
        }
    }
}

impl Error for MemoryAccessError {}

/// Synchronous checked bulk access to a modeled byte store.
pub trait MemoryAccess {
    /// Checks a complete extent without reading, writing, allocating, or changing storage.
    ///
    /// Errors follow overflow, alignment, lookup, then region-crossing order.
    /// A zero-length extent accepts installed region endpoints, but not unmapped gaps.
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError>;

    /// Reads one contiguous slice into caller-owned storage.
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError>;

    /// Writes one contiguous caller-owned slice.
    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError>;
}
