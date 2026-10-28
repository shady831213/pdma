use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::memory::{MemoryAddress, MemoryAlignment};
use crate::memory::{MemoryRegion, StorageError};

/// Failure to construct a storage region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryRegionError {
    /// Storage regions cannot be empty.
    Empty,
    /// The base does not meet the declared alignment.
    MisalignedBase {
        /// Rejected base address.
        base: MemoryAddress,
        /// Required alignment.
        alignment: MemoryAlignment,
    },
    /// The exclusive region end exceeds the address space.
    AddressOverflow {
        /// Region base address.
        base: MemoryAddress,
        /// Requested byte length.
        length: u64,
    },
    /// The byte length cannot be represented by host storage.
    StorageSizeOutOfRange {
        /// Requested byte length.
        length: u64,
    },
}

impl Display for MemoryRegionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("memory storage region is empty"),
            Self::MisalignedBase { base, alignment } => write!(
                formatter,
                "memory region base {:#x} is not aligned to {}",
                base.raw(),
                alignment.raw()
            ),
            Self::AddressOverflow { base, length } => write!(
                formatter,
                "memory region at {:#x} with length {length} exceeds the address space",
                base.raw()
            ),
            Self::StorageSizeOutOfRange { length } => {
                write!(
                    formatter,
                    "memory region length {length} exceeds host storage size"
                )
            }
        }
    }
}

impl Error for MemoryRegionError {}

/// Failure to atomically add storage regions.
#[derive(Debug, PartialEq, Eq)]
pub enum MemoryError {
    /// The backing store rejected access before installation began.
    Storage(StorageError),
    /// Backing storage could not reserve the requested byte length.
    StorageExhausted {
        /// Requested byte length.
        length: u64,
    },
    /// The final region table could not reserve enough entries.
    RegionTableExhausted {
        /// Required total entry count.
        entries: usize,
    },
    /// Two regions overlap.
    Overlap {
        /// Earlier installed or proposed region.
        first: MemoryRegion,
        /// Later proposed region.
        second: MemoryRegion,
    },
}

impl Display for MemoryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => Display::fmt(error, formatter),
            Self::StorageExhausted { length } => {
                write!(
                    formatter,
                    "unable to reserve {length} bytes of memory storage"
                )
            }
            Self::RegionTableExhausted { entries } => {
                write!(
                    formatter,
                    "unable to reserve a memory table with {entries} regions"
                )
            }
            Self::Overlap { first, second } => write!(
                formatter,
                "memory regions at {:#x} and {:#x} overlap",
                first.base().raw(),
                second.base().raw()
            ),
        }
    }
}

impl Error for MemoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::StorageExhausted { .. }
            | Self::RegionTableExhausted { .. }
            | Self::Overlap { .. } => None,
        }
    }
}

/// Failure of one checked contiguous access to the built-in memory store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryAccessError {
    /// The backing store rejected access before the operation ran.
    Storage(StorageError),
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
            Self::Storage(error) => Display::fmt(error, formatter),
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

impl Error for MemoryAccessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::AddressOverflow { .. }
            | Self::Misaligned { .. }
            | Self::Unmapped { .. }
            | Self::CrossesRegion { .. } => None,
        }
    }
}
