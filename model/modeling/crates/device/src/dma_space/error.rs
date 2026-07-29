use core::fmt;

/// Failure while staging, committing, or accessing checked DMA storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaSpaceError {
    /// Fixed-width allocation arithmetic or address space was exhausted.
    AddressExhausted,
    /// Host memory could not be reserved for the requested batch.
    StorageExhausted,
    /// A committed or proposed region overlaps another reserved extent.
    OverlappingRegion,
    /// A supplied region is not an aligned exact committed mapping.
    InvalidRegion,
    /// A stage was created from an allocator generation that is no longer current.
    StaleStage {
        /// Generation observed while staging.
        expected_generation: u64,
        /// Current allocator generation.
        actual_generation: u64,
    },
    /// No mapped region contains the requested starting IOVA.
    Unmapped,
    /// A checked access begins in one region and ends beyond it.
    CrossesRegion,
}

impl fmt::Display for DmaSpaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressExhausted => f.write_str("DMA address space is exhausted"),
            Self::StorageExhausted => f.write_str("DMA backing storage is exhausted"),
            Self::OverlappingRegion => f.write_str("DMA region overlaps a reserved extent"),
            Self::InvalidRegion => f.write_str("DMA region is not an exact aligned mapping"),
            Self::StaleStage {
                expected_generation,
                actual_generation,
            } => write!(
                f,
                "DMA stage generation {expected_generation} is stale at generation {actual_generation}"
            ),
            Self::Unmapped => f.write_str("IOVA is not mapped"),
            Self::CrossesRegion => f.write_str("IOVA access crosses a region boundary"),
        }
    }
}

impl std::error::Error for DmaSpaceError {}
