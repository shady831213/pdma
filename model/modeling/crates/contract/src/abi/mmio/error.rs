use super::MmioByteCount;

/// A closed structural MMIO transaction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioAccessError {
    /// The requested extent is outside the 4 KiB region.
    UnmappedOffset {
        /// Requested starting offset.
        offset: u32,
    },
    /// The aligned in-range starting offset is not registered.
    ReservedOffset {
        /// Requested starting offset.
        offset: u32,
    },
    /// The starting offset is not aligned to the requested width.
    Misaligned {
        /// Requested starting offset.
        offset: u32,
        /// Requested transaction width.
        width: MmioByteCount,
    },
    /// The requested width is unsupported or does not match the register.
    WrongWidth {
        /// Requested starting offset.
        offset: u32,
        /// Required width, or zero for an unsupported width.
        expected: MmioByteCount,
        /// Requested transaction width.
        actual: MmioByteCount,
    },
    /// A write-only register was read.
    ReadFromWriteOnly {
        /// Requested starting offset.
        offset: u32,
    },
    /// A read-only register was written.
    WriteToReadOnly {
        /// Requested starting offset.
        offset: u32,
    },
    /// A write set bits outside the register's writable mask.
    ReservedBits {
        /// Requested starting offset.
        offset: u32,
        /// Bits outside the writable mask.
        bits: u64,
    },
}

impl core::fmt::Display for MmioAccessError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "MMIO access failed: {self:?}")
    }
}

impl std::error::Error for MmioAccessError {}
