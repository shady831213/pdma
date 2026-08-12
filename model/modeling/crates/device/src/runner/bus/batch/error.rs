use crate::dma_space::DmaSpaceError;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioResult, MmioResultDetails};
use pdma_model_contract::{DmaAddress, DmaRegion};

/// Attempted extent used to retain contextual DMA failure details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmaBatchContext {
    /// No attempted extent is meaningful for this failure.
    None,
    /// One attempted checked DMA access.
    Access {
        /// First byte of the attempted access.
        address: DmaAddress,
        /// Exact attempted byte count.
        length: u64,
    },
    /// One attempted DMA mapping.
    Region(DmaRegion),
}

/// Closed failure surface for the two ordered queue batches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MmioBatchError {
    /// The typed request does not satisfy its one closed grammar.
    InvalidShape,
    /// One fixed MMIO access is structurally invalid.
    Access(MmioAccessError),
    /// DMA projection failed with an exact numeric result and context.
    Dma(MmioResultDetails),
    /// The terminal ordinary queue command would return non-OK.
    Command(MmioResultDetails),
}

impl MmioBatchError {
    /// Maps every checked DMA failure using the attempted operation context.
    pub const fn from_dma(error: DmaSpaceError, context: DmaBatchContext) -> Self {
        let details = match (error, context) {
            (
                DmaSpaceError::Unmapped | DmaSpaceError::CrossesRegion,
                DmaBatchContext::Access { address, length },
            ) => [address.raw(), length, 0],
            (
                DmaSpaceError::InvalidRegion | DmaSpaceError::OverlappingRegion,
                DmaBatchContext::Region(region),
            ) => [region.base().raw(), region.length(), 0],
            (
                DmaSpaceError::StaleStage {
                    expected_generation,
                    actual_generation,
                },
                _,
            ) => [expected_generation, actual_generation, 0],
            (
                DmaSpaceError::AddressExhausted
                | DmaSpaceError::StorageExhausted
                | DmaSpaceError::Unmapped
                | DmaSpaceError::CrossesRegion
                | DmaSpaceError::InvalidRegion
                | DmaSpaceError::OverlappingRegion,
                _,
            ) => [0; 3],
        };
        let result = match error {
            DmaSpaceError::AddressExhausted => MmioResult::DmaAddressExhausted,
            DmaSpaceError::StorageExhausted => MmioResult::DmaStorageExhausted,
            DmaSpaceError::OverlappingRegion => MmioResult::DmaOverlap,
            DmaSpaceError::InvalidRegion => MmioResult::DmaInvalidRegion,
            DmaSpaceError::StaleStage { .. } => MmioResult::DmaStaleStage,
            DmaSpaceError::Unmapped => MmioResult::DmaUnmapped,
            DmaSpaceError::CrossesRegion => MmioResult::DmaCrossesRegion,
        };
        Self::Dma(MmioResultDetails { result, details })
    }
}

impl core::fmt::Display for MmioBatchError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "MMIO batch failed: {self:?}")
    }
}

impl std::error::Error for MmioBatchError {}
