use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{MemoryAddress, MemoryAlignment, MemoryRegion, StorageError};

mod unit {
    pub trait UnitSideband: Sized {
        fn unit() -> Self;
    }

    impl UnitSideband for () {
        fn unit() -> Self {}
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

/// Synchronous checked bulk access to a modeled byte store.
pub trait MemoryAccess<S = ()> {
    /// Backend-owned failure, preserved by the default access helpers.
    type Error;

    /// Performs one indivisible 32-bit access, or rejects the unsupported capability.
    /// Implementations must honor [`crate::AtomicOperation`]'s failure and ordering contract.
    fn atomic32_with_sideband(
        &self,
        _sideband: &S,
        _address: MemoryAddress,
        _operation: crate::AtomicOperation<u32>,
    ) -> Result<u32, crate::AtomicAccessError<Self::Error>> {
        Err(crate::AtomicAccessError::Unsupported)
    }

    /// Performs one indivisible 64-bit access; errors must precede modification.
    fn atomic64_with_sideband(
        &self,
        _sideband: &S,
        _address: MemoryAddress,
        _operation: crate::AtomicOperation<u64>,
    ) -> Result<u64, crate::AtomicAccessError<Self::Error>> {
        Err(crate::AtomicAccessError::Unsupported)
    }

    /// Performs an indivisible word access without metadata on a unit-sideband backend.
    fn atomic32(
        &self,
        address: MemoryAddress,
        operation: crate::AtomicOperation<u32>,
    ) -> Result<u32, crate::AtomicAccessError<Self::Error>>
    where
        S: unit::UnitSideband,
    {
        self.atomic32_with_sideband(&S::unit(), address, operation)
    }

    /// Performs an indivisible double-word access on a unit-sideband backend.
    fn atomic64(
        &self,
        address: MemoryAddress,
        operation: crate::AtomicOperation<u64>,
    ) -> Result<u64, crate::AtomicAccessError<Self::Error>>
    where
        S: unit::UnitSideband,
    {
        self.atomic64_with_sideband(&S::unit(), address, operation)
    }

    /// Checks a complete extent without reading, writing, allocating, or changing storage.
    ///
    /// The backend defines accepted extents and validation errors.
    fn validate_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error>;

    /// Reads one contiguous slice into caller-owned storage.
    fn read_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error>;

    /// Writes one contiguous caller-owned slice.
    fn write_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error>;

    /// Checks an extent on a unit-sideband backend.
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error>
    where
        S: unit::UnitSideband,
    {
        self.validate_with_sideband(&S::unit(), address, alignment, length)
    }

    /// Reads bytes from a unit-sideband backend.
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error>
    where
        S: unit::UnitSideband,
    {
        self.read_with_sideband(&S::unit(), address, alignment, destination)
    }

    /// Writes bytes to a unit-sideband backend.
    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error>
    where
        S: unit::UnitSideband,
    {
        self.write_with_sideband(&S::unit(), address, alignment, source)
    }

    /// Reads into a default-constructed byte buffer and returns its owner.
    ///
    /// The buffer's slice length determines the access extent. This does not
    /// decode values, infer alignment, or reinterpret Rust object layout.
    fn read_typed<T>(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
    ) -> Result<T, Self::Error>
    where
        Self: Sized,
        S: unit::UnitSideband,
        T: Default + AsMut<[u8]>,
    {
        self.read_typed_with_sideband(&S::unit(), address, alignment)
    }

    /// Writes a value's exposed bytes without encoding or reinterpreting it.
    fn write_typed<T>(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
        S: unit::UnitSideband,
        T: AsRef<[u8]> + ?Sized,
    {
        self.write_typed_with_sideband(&S::unit(), address, alignment, value)
    }

    /// Reads a byte-buffer owner using explicit access metadata.
    fn read_typed_with_sideband<T>(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
    ) -> Result<T, Self::Error>
    where
        Self: Sized,
        T: Default + AsMut<[u8]>,
    {
        let mut value = T::default();
        self.read_with_sideband(sideband, address, alignment, value.as_mut())?;
        Ok(value)
    }

    /// Writes exposed bytes using explicit access metadata.
    fn write_typed_with_sideband<T>(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
        T: AsRef<[u8]> + ?Sized,
    {
        self.write_with_sideband(sideband, address, alignment, value.as_ref())
    }
}
