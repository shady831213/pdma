//! Exact ABI queue activation, publication, and batch transactions.

pub mod batch;
#[cfg(test)]
pub(crate) use batch::BatchPreparationFault;
pub mod control;

use crate::dma::{PdmaMemoryError, PdmaMemoryStage};
use pdma_model_contract::abi::{AbiControlError, DmaAddress};
use pdma_model_contract::semantic::{DeviceEpoch, RingCursor, SemanticError};
use std::fmt;

/// One byte-opaque DMA write committed by a bus transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaWrite {
    address: DmaAddress,
    bytes: Vec<u8>,
}

impl DmaWrite {
    /// Creates a write at an exact mapped address.
    pub const fn new(address: DmaAddress, bytes: Vec<u8>) -> Self {
        Self { address, bytes }
    }

    pub(super) const fn address(&self) -> DmaAddress {
        self.address
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One all-or-none host submission acceptance request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionAcceptance {
    pub(super) queue: pdma_model_contract::semantic::EpochQueue,
    pub(super) expected_producer: RingCursor,
    pub(super) descriptor: DmaWrite,
    pub(super) indirect: Option<(PdmaMemoryStage, DmaWrite)>,
}

impl SubmissionAcceptance {
    /// Creates an acceptance request from already prepared opaque bytes.
    pub const fn new(
        queue: pdma_model_contract::semantic::EpochQueue,
        expected_producer: RingCursor,
        descriptor: DmaWrite,
        indirect: Option<(PdmaMemoryStage, DmaWrite)>,
    ) -> Self {
        Self {
            queue,
            expected_producer,
            descriptor,
            indirect,
        }
    }
}

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

pub use batch::{
    DmaBatchContext, PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch,
};
pub use control::QueueStatus;

/// One decoded DMA result cached before result-ring publication is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDmaResult {
    /// Successful execution waiting for completion-ring publication.
    Completion(pdma_model_contract::semantic::CompletionRecord),
    /// Canonical submission fault waiting for event-ring publication.
    Event(pdma_model_contract::abi::AbiEvent),
}
