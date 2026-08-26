//! Transitional semantic device compatibility surfaces retained through Task 25.

mod reset;

pub use crate::error::{ModelError, ModelResult};
pub use crate::runner::SyncDevice;

use crate::{PdmaControlError, PdmaMemoryError, PdmaRunError};

/// Named additive adapter to the legacy `ModelError` surface, removable in Task 26.
#[derive(Debug)]
pub struct ModelErrorAdapter;

impl ModelErrorAdapter {
    /// Projects a memory failure into the historical compatibility vocabulary.
    pub const fn from_memory(error: PdmaMemoryError) -> ModelError {
        match error {
            PdmaMemoryError::AddressExhausted | PdmaMemoryError::StorageExhausted => {
                ModelError::DmaAllocationExhausted
            }
            PdmaMemoryError::OverlappingRegion
            | PdmaMemoryError::InvalidRegion
            | PdmaMemoryError::StaleStage { .. }
            | PdmaMemoryError::Unmapped
            | PdmaMemoryError::CrossesRegion => ModelError::DmaSubmissionState,
        }
    }

    /// Projects a control failure into the historical compatibility vocabulary.
    pub const fn from_control(
        error: PdmaControlError,
        queue: pdma_model_contract::semantic::QueueId,
    ) -> ModelError {
        match error {
            PdmaControlError::StaleEpoch { expected, actual } => {
                ModelError::StaleDeviceEpoch { expected, actual }
            }
            PdmaControlError::InvalidQueue => ModelError::UnknownQueue(queue),
            PdmaControlError::Dma(memory) => Self::from_memory(memory),
            PdmaControlError::Abi(_) | PdmaControlError::InvalidTransaction => {
                ModelError::DmaSubmissionState
            }
        }
    }

    /// Removes the canonical run wrapper for legacy consumers.
    pub const fn from_run(error: PdmaRunError) -> ModelError {
        match error {
            PdmaRunError::Semantic(error) => error,
        }
    }
}
