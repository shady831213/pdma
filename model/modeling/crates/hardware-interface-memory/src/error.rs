use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{MemoryAddress, MemoryAlignment, MemoryRegion, StorageError};

/// Failure to construct a checked alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryAlignmentError {
    /// The value is zero or is not a power of two.
    Invalid {
        /// Rejected numeric alignment.
        alignment: u64,
    },
}

impl Display for MemoryAlignmentError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { alignment } => {
                write!(
                    formatter,
                    "memory alignment {alignment} is not a nonzero power of two"
                )
            }
        }
    }
}

impl Error for MemoryAlignmentError {}

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
