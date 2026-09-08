/// Lossless domain for raw register transaction byte counts.
pub type MmioByteCount = u128;

/// Structural failure of one direct hardware register access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioAccessError {
    /// The requested transaction crosses the u32 address aperture.
    UnmappedOffset {
        /// Requested starting offset.
        offset: u32,
    },
    /// The aligned starting offset is not a generated register start.
    ReservedOffset {
        /// Requested starting offset.
        offset: u32,
    },
    /// The starting offset is not aligned to the transaction width.
    Misaligned {
        /// Requested starting offset.
        offset: u32,
        /// Requested transaction width.
        width: MmioByteCount,
    },
    /// The transaction width is unsupported or differs from the register width.
    WrongWidth {
        /// Requested starting offset.
        offset: u32,
        /// Register width, or zero when the transaction width is unsupported.
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
    /// A write sets bits outside the generated writable mask.
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
