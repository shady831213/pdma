//! Atomic DMA-backed queue control transactions.

mod batch;
#[cfg(test)]
pub(super) use batch::BatchPreparationFault;
pub(super) mod control;

use super::{AsyncDeviceHandle, publish, wake};
use crate::dma_space::{PdmaMemoryError, PdmaMemoryStage};
use pdma_model_contract::{
    AbiControlError, DeviceEpoch, DmaAddress, QueueActivation, QueueControl, RingCursor,
};
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
pub enum BusControlError {
    /// DMA staging or access validation failed.
    Dma(PdmaMemoryError),
    /// Typed ABI cursor or layout validation failed.
    Abi(AbiControlError),
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

impl fmt::Display for BusControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dma(error) => write!(formatter, "{error}"),
            Self::Abi(error) => write!(formatter, "{error}"),
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

impl std::error::Error for BusControlError {}

impl From<PdmaMemoryError> for BusControlError {
    fn from(error: PdmaMemoryError) -> Self {
        Self::Dma(error)
    }
}

impl From<AbiControlError> for BusControlError {
    fn from(error: AbiControlError) -> Self {
        Self::Abi(error)
    }
}

pub use batch::{
    DmaBatchContext, MmioBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch,
};
pub use control::QueueStatus;

impl super::AsyncRunSnapshot {
    /// Returns the publication generation represented by this snapshot.
    pub const fn generation(self) -> usize {
        self.generation
    }
}

impl AsyncDeviceHandle {
    /// Atomically commits three staged ring mappings and one queue activation.
    pub async fn activate_queue(
        &self,
        proposal: PdmaMemoryStage,
        activation: QueueActivation,
    ) -> Result<QueueStatus, BusControlError> {
        let (result, generation) = {
            let mut runner = self.shared.state.lock().await;
            let result = control::activate_staged(&mut runner, proposal, activation);
            if result.is_ok() {
                wake(&mut runner);
            }
            (result, runner.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Activates one queue over three exact mappings already committed in DMA space.
    pub async fn activate_mapped_queue(
        &self,
        activation: QueueActivation,
    ) -> Result<QueueStatus, BusControlError> {
        let (result, generation) = {
            let mut runner = self.shared.state.lock().await;
            let result = control::activate_mapped(&mut runner, activation);
            if result.is_ok() {
                wake(&mut runner);
            }
            (result, runner.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Atomically commits optional indirect DMA, opaque writes, and producer advance.
    pub async fn accept_submission(
        &self,
        request: SubmissionAcceptance,
    ) -> Result<QueueStatus, BusControlError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = control::transactions::accept_submission(&mut state, request);
            if result.is_ok() {
                wake(&mut state);
            }
            (result, state.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Executes one typed queue status or cursor transaction.
    pub async fn control_queue(
        &self,
        request: QueueControl,
    ) -> Result<QueueStatus, BusControlError> {
        let (result, changed, generation) = {
            let mut state = self.shared.state.lock().await;
            let (result, changed) = control::transactions::control(&mut state, request);
            if changed {
                wake(&mut state);
            }
            (result, changed, state.generation)
        };
        if changed {
            publish(&self.shared, generation);
        }
        result
    }
}
