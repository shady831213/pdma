//! Opaque fixed-width IOVA values.

use super::{AbiAddressError, AbiDecodeError, AbiEncodeError};

/// Fixed-width ABI identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiId(u64);

impl AbiId {
    /// Creates an identifier from its exact wire representation.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the fixed-width identifier value.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Converts a wire identifier to the model's platform-sized ID vocabulary.
    pub fn try_into_model(self) -> Result<usize, AbiDecodeError> {
        usize::try_from(self.0).map_err(|_| AbiDecodeError::UnrepresentableId { value: self.0 })
    }
}

impl TryFrom<u128> for AbiId {
    type Error = AbiEncodeError;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        u64::try_from(value)
            .map(Self)
            .map_err(|_| AbiEncodeError::UnrepresentableId { value })
    }
}

impl TryFrom<usize> for AbiId {
    type Error = AbiEncodeError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        u64::try_from(value)
            .map(Self)
            .map_err(|_| AbiEncodeError::UnrepresentableModelId { value })
    }
}

/// Fixed-width I/O virtual address used in ABI records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DmaAddress(u64);

impl DmaAddress {
    /// Creates an address from its exact fixed-width representation.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the fixed-width IOVA value.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Adds a fixed-width byte offset without wrapping.
    pub const fn checked_add(self, length: u64) -> Result<Self, AbiAddressError> {
        match self.0.checked_add(length) {
            Some(end) => Ok(Self(end)),
            None => Err(AbiAddressError::RangeOverflow {
                base: self.0,
                length,
            }),
        }
    }
}

impl TryFrom<u128> for DmaAddress {
    type Error = AbiAddressError;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        u64::try_from(value)
            .map(Self)
            .map_err(|_| AbiAddressError::UnrepresentableAddress { value })
    }
}
