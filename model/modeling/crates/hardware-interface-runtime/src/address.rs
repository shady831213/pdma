use crate::MemoryAlignmentError;

/// An opaque byte address in a modeled memory space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryAddress(u64);

impl MemoryAddress {
    /// Creates an address from its numeric representation.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the numeric representation.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Returns the checked address after adding a byte length.
    pub const fn checked_add(self, length: u64) -> Option<Self> {
        match self.0.checked_add(length) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }
}

impl From<u64> for MemoryAddress {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

/// A checked nonzero power-of-two address alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryAlignment(u64);

impl MemoryAlignment {
    /// Byte alignment accepted by every address.
    pub const ONE: Self = Self(1);

    /// Parses a numeric alignment.
    pub const fn try_new(alignment: u64) -> Result<Self, MemoryAlignmentError> {
        if alignment.is_power_of_two() {
            Ok(Self(alignment))
        } else {
            Err(MemoryAlignmentError::Invalid { alignment })
        }
    }

    /// Returns the numeric alignment.
    pub const fn raw(self) -> u64 {
        self.0
    }

    pub(crate) const fn accepts(self, address: MemoryAddress) -> bool {
        address.raw() & (self.0 - 1) == 0
    }
}
