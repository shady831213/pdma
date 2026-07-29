//! Opaque IOVA and model-only DMA-region capabilities.

use crate::ids::BufferId;

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

/// Model-only correlation between a semantic buffer and its IOVA extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaRegion {
    object: BufferId,
    base: DmaAddress,
    length: u64,
    exclusive_end: DmaAddress,
}

impl DmaRegion {
    /// Creates a region whose exclusive end is representable.
    pub const fn try_new(
        object: BufferId,
        base: DmaAddress,
        length: u64,
    ) -> Result<Self, AbiAddressError> {
        match base.checked_add(length) {
            Ok(exclusive_end) => Ok(Self {
                object,
                base,
                length,
                exclusive_end,
            }),
            Err(error) => Err(error),
        }
    }

    /// Returns the semantic DMA object used only by the model.
    pub const fn object(self) -> BufferId {
        self.object
    }

    /// Returns the region's base IOVA.
    pub const fn base(self) -> DmaAddress {
        self.base
    }

    /// Returns the region length in fixed-width bytes.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Returns the checked exclusive end of the region.
    pub const fn exclusive_end(self) -> DmaAddress {
        self.exclusive_end
    }

    /// Returns whether the region contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}
