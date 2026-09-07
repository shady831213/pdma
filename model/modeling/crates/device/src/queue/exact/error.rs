use crate::memory::PdmaMemoryError;
use pdma_model_contract::abi::AbiControlError;
use pdma_model_contract::semantic::{DeviceEpoch, SemanticError};
use std::fmt;

/// Typed queue-control transaction failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdmaControlError {
    /// DMA staging or access validation failed.
    Dma(PdmaMemoryError),
    /// Exact ABI layout or numeric control validation failed.
    Abi(AbiControlError),
    /// Semantic queue state validation failed.
    Semantic(SemanticError),
    /// The supplied device epoch differs from the current device epoch.
    StaleEpoch {
        /// Current device epoch.
        expected: DeviceEpoch,
        /// Supplied stale capability epoch.
        actual: DeviceEpoch,
    },
    /// The queue identity does not name the next or active queue.
    InvalidQueue,
    /// A staged region, write, or queue layout does not match the transaction.
    InvalidTransaction,
}

impl fmt::Display for PdmaControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dma(error) => write!(formatter, "{error}"),
            Self::Abi(error) => write!(formatter, "{error}"),
            Self::Semantic(error) => write!(formatter, "{error}"),
            Self::StaleEpoch { expected, actual } => {
                write!(
                    formatter,
                    "stale device epoch: expected {expected:?}, actual {actual:?}"
                )
            }
            Self::InvalidQueue => formatter.write_str("invalid queue identity"),
            Self::InvalidTransaction => formatter.write_str("invalid atomic queue transaction"),
        }
    }
}

impl std::error::Error for PdmaControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dma(error) => Some(error),
            Self::Abi(error) => Some(error),
            Self::Semantic(error) => Some(error),
            Self::StaleEpoch { .. } | Self::InvalidQueue | Self::InvalidTransaction => None,
        }
    }
}

impl From<PdmaMemoryError> for PdmaControlError {
    fn from(error: PdmaMemoryError) -> Self {
        Self::Dma(error)
    }
}

impl From<AbiControlError> for PdmaControlError {
    fn from(error: AbiControlError) -> Self {
        Self::Abi(error)
    }
}

impl From<SemanticError> for PdmaControlError {
    fn from(error: SemanticError) -> Self {
        Self::Semantic(error)
    }
}
