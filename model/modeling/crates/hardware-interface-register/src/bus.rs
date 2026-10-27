use std::error::Error;
use std::fmt::{self, Display, Formatter};

use hardware_interface_schema::RegisterMapLayout;

use super::{
    RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterBusOffset, RegisterBusWidth,
    RegisterImage,
};
use hardware_interface_storage::{StorageBackend, StorageError};

/// Canonical leaf-validation or storage failure after address translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterEndpointError {
    /// No leaf begins at the translated offset.
    UnmappedOffset {
        /// Canonical translated offset.
        offset: RegisterBusOffset,
    },
    /// The caller width differs from the leaf width.
    WidthMismatch {
        /// Canonical translated offset.
        offset: RegisterBusOffset,
        /// Declared leaf width.
        expected: usize,
        /// Caller-provided width.
        actual: usize,
    },
    /// Software attempted to read a write-only leaf.
    ReadForbidden {
        /// Canonical translated offset.
        offset: RegisterBusOffset,
    },
    /// Software attempted to write a read-only leaf.
    WriteForbidden {
        /// Canonical translated offset.
        offset: RegisterBusOffset,
    },
    /// A write attempted to change non-writable bits.
    ReservedBitChange {
        /// Canonical translated offset.
        offset: RegisterBusOffset,
        /// Declared writable bits.
        writable_mask: u64,
        /// Attempted changes outside the writable mask.
        changed: u64,
    },
    /// Resolved metadata did not fit its declared root image.
    InvalidLayout {
        /// Canonical translated offset.
        offset: RegisterBusOffset,
    },
    /// The storage policy rejected the operation.
    Storage(StorageError),
}

impl Display for RegisterEndpointError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "register endpoint error: {self:?}")
    }
}

impl Error for RegisterEndpointError {}

/// Addressed raw access to one authoritative typed register image.
#[derive(Debug, Clone)]
pub struct RegisterBus<M, B>
where
    M: RegisterMapLayout,
    B: StorageBackend<Vec<u8>>,
{
    base: RegisterBusBase,
    image: RegisterImage<M, B>,
}

impl<M, B> RegisterBus<M, B>
where
    M: RegisterMapLayout,
    B: StorageBackend<Vec<u8>>,
{
    /// Binds an encoded image to one full-width bus base.
    pub const fn new(base: RegisterBusBase, image: RegisterImage<M, B>) -> Self {
        Self { base, image }
    }

    /// Returns the full-width base used for every translation.
    pub const fn base(&self) -> RegisterBusBase {
        self.base
    }

    /// Reads one exact-start leaf without changing `destination` on failure.
    pub fn read(
        &self,
        address: RegisterBusAddress,
        destination: &mut [u8],
    ) -> Result<(), RegisterBusError<RegisterEndpointError>> {
        let width = RegisterBusWidth::new(destination.len());
        let offset = self.translate(address, width)?;
        self.image
            .read_leaf(offset, destination)
            .map_err(|source| endpoint(self.base, address, width, source))
    }

    /// Writes one exact-start leaf after complete scratch validation.
    pub fn write(
        &self,
        address: RegisterBusAddress,
        source: &[u8],
    ) -> Result<(), RegisterBusError<RegisterEndpointError>> {
        let width = RegisterBusWidth::new(source.len());
        let offset = self.translate(address, width)?;
        self.image
            .write_leaf(offset, source)
            .map_err(|source| endpoint(self.base, address, width, source))
    }

    fn translate(
        &self,
        address: RegisterBusAddress,
        width: RegisterBusWidth,
    ) -> Result<RegisterBusOffset, RegisterBusError<RegisterEndpointError>> {
        address
            .offset_from(self.base, width)
            .map_err(RegisterBusError::from_translation)
    }
}

const fn endpoint(
    base: RegisterBusBase,
    address: RegisterBusAddress,
    width: RegisterBusWidth,
    source: RegisterEndpointError,
) -> RegisterBusError<RegisterEndpointError> {
    RegisterBusError::Endpoint {
        base,
        address,
        width,
        source,
    }
}
