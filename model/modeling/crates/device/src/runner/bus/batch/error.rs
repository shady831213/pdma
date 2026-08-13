use crate::dma_space::PdmaMemoryError;
use hardware_interface_runtime::ObservationGateError;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioResult, MmioResultDetails};
use pdma_model_contract::{DmaAddress, PdmaMemoryRegion};

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
    Region(PdmaMemoryRegion),
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
    /// The core-owned ordered observation gate rejected reservation or emission.
    Observation(ObservationGateError),
}

impl From<ObservationGateError> for MmioBatchError {
    fn from(error: ObservationGateError) -> Self {
        Self::Observation(error)
    }
}

impl MmioBatchError {
    /// Maps every checked DMA failure using the attempted operation context.
    pub const fn from_dma(error: PdmaMemoryError, context: DmaBatchContext) -> Self {
        let details = match (error, context) {
            (
                PdmaMemoryError::Unmapped | PdmaMemoryError::CrossesRegion,
                DmaBatchContext::Access { address, length },
            ) => [address.raw(), length, 0],
            (
                PdmaMemoryError::InvalidRegion | PdmaMemoryError::OverlappingRegion,
                DmaBatchContext::Region(region),
            ) => [region.base().raw(), region.length(), 0],
            (
                PdmaMemoryError::StaleStage {
                    expected_generation,
                    actual_generation,
                },
                _,
            ) => [expected_generation, actual_generation, 0],
            (
                PdmaMemoryError::AddressExhausted
                | PdmaMemoryError::StorageExhausted
                | PdmaMemoryError::Unmapped
                | PdmaMemoryError::CrossesRegion
                | PdmaMemoryError::InvalidRegion
                | PdmaMemoryError::OverlappingRegion,
                _,
            ) => [0; 3],
        };
        let result = match error {
            PdmaMemoryError::AddressExhausted => MmioResult::DmaAddressExhausted,
            PdmaMemoryError::StorageExhausted => MmioResult::DmaStorageExhausted,
            PdmaMemoryError::OverlappingRegion => MmioResult::DmaOverlap,
            PdmaMemoryError::InvalidRegion => MmioResult::DmaInvalidRegion,
            PdmaMemoryError::StaleStage { .. } => MmioResult::DmaStaleStage,
            PdmaMemoryError::Unmapped => MmioResult::DmaUnmapped,
            PdmaMemoryError::CrossesRegion => MmioResult::DmaCrossesRegion,
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
