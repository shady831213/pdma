//! Opaque fixed-width DMA address values.

use super::AbiAddressError;

/// Fixed-width DMA address used in ABI records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DmaAddress(u64);

impl DmaAddress {
    /// Creates an address from its exact fixed-width representation.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the fixed-width DMA address value.
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

/// Exact fixed-width DMA address extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaRegion {
    base: DmaAddress,
    length: u64,
    exclusive_end: DmaAddress,
}

impl DmaRegion {
    /// Creates an extent whose exclusive end is representable.
    pub const fn try_new(base: DmaAddress, length: u64) -> Result<Self, AbiAddressError> {
        match base.checked_add(length) {
            Ok(exclusive_end) => Ok(Self {
                base,
                length,
                exclusive_end,
            }),
            Err(error) => Err(error),
        }
    }

    /// Exact base DMA address.
    pub const fn base(self) -> DmaAddress {
        self.base
    }

    /// Fixed-width byte length.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Checked exclusive end address.
    pub const fn exclusive_end(self) -> DmaAddress {
        self.exclusive_end
    }

    /// Whether the extent contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}
