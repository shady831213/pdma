use crate::{MemoryAddress, MemoryAlignment, MemoryRegionError};

/// Metadata for one checked, non-empty storage region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemoryRegion {
    base: MemoryAddress,
    length: u64,
    alignment: MemoryAlignment,
    end: MemoryAddress,
    storage_length: usize,
}

impl MemoryRegion {
    /// Constructs checked region metadata.
    pub fn try_new(
        base: MemoryAddress,
        length: u64,
        alignment: MemoryAlignment,
    ) -> Result<Self, MemoryRegionError> {
        if length == 0 {
            return Err(MemoryRegionError::Empty);
        }
        if !alignment.accepts(base) {
            return Err(MemoryRegionError::MisalignedBase { base, alignment });
        }
        let end = base
            .checked_add(length)
            .ok_or(MemoryRegionError::AddressOverflow { base, length })?;
        let storage_length = usize::try_from(length)
            .map_err(|_| MemoryRegionError::StorageSizeOutOfRange { length })?;
        Ok(Self {
            base,
            length,
            alignment,
            end,
            storage_length,
        })
    }

    /// Returns the inclusive region base.
    pub const fn base(self) -> MemoryAddress {
        self.base
    }

    /// Returns the nonzero byte length.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Returns the declared base alignment.
    pub const fn alignment(self) -> MemoryAlignment {
        self.alignment
    }

    pub(crate) const fn storage_length(self) -> usize {
        self.storage_length
    }

    pub(crate) const fn end(self) -> MemoryAddress {
        self.end
    }

    pub(crate) const fn overlaps(self, other: Self) -> bool {
        self.base.raw() < other.end().raw() && other.base.raw() < self.end().raw()
    }
}
