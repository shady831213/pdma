//! Exact register endpoint and transaction preparation.

use super::mmio::dispatch;
use super::mmio::queue::{BootstrapBatchStaging, SubmissionBatchStaging};
use crate::device::DeviceCore;
use hardware_interface_runtime::{AccessOrigin, ObservationGateError, RegisterObservation};
use pdma_model_contract::abi::mmio::QueueResponseBank;
use pdma_model_contract::abi::mmio::{MmioAccessError, RegisterId, decode_write};
use std::fmt::{self, Display, Formatter};

#[doc(hidden)]
pub mod claim;
pub(crate) use claim::prepare_claim;

/// Failure from an exact generated-view register operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmioRegisterError {
    /// The generated PDMA register map rejected the transaction.
    Access(MmioAccessError),
    /// The core-owned ordered observer gate rejected committed emission.
    Observation(ObservationGateError),
}

impl Display for MmioRegisterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => Display::fmt(error, formatter),
            Self::Observation(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for MmioRegisterError {}

impl From<MmioAccessError> for MmioRegisterError {
    fn from(error: MmioAccessError) -> Self {
        Self::Access(error)
    }
}

impl From<ObservationGateError> for MmioRegisterError {
    fn from(error: ObservationGateError) -> Self {
        Self::Observation(error)
    }
}

pub(crate) struct PreparedRead {
    pub(crate) response: Vec<u8>,
    offset: u32,
}

impl PreparedRead {
    pub(crate) fn observation(&self, sequence: u64) -> RegisterObservation {
        RegisterObservation::read(sequence, AccessOrigin::Direct, self.offset, &self.response)
    }
}

#[derive(Clone)]
pub(crate) struct PreparedWrite {
    register: RegisterId,
    value: u64,
    offset: u32,
    request: Vec<u8>,
}

impl PreparedWrite {
    pub(crate) fn commit(&self, state: &mut DeviceCore) {
        dispatch::write(state, self.register, self.value);
    }

    pub(crate) fn observation(&self, sequence: u64, origin: AccessOrigin) -> RegisterObservation {
        RegisterObservation::write(sequence, origin, self.offset, &self.request)
    }
}

pub(super) struct PreparedBatchWrite {
    write: PreparedWrite,
    mutation: PreparedBatchRegisterMutation,
}

#[derive(Clone, Copy)]
pub(super) enum BatchQueueCommand {
    Activate,
    PublishSubmissionProducer,
}

enum PreparedBatchRegisterMutation {
    Bootstrap(BootstrapBatchStaging, QueueResponseBank),
    Submission(SubmissionBatchStaging, QueueResponseBank),
}

impl BatchQueueCommand {
    const fn code(self) -> u32 {
        match self {
            Self::Activate => 1,
            Self::PublishSubmissionProducer => 3,
        }
    }
}

impl PreparedBatchWrite {
    pub(super) fn commit(self, state: &mut DeviceCore) -> PreparedWrite {
        match self.mutation {
            PreparedBatchRegisterMutation::Bootstrap(staging, response) => {
                state
                    .registers
                    .queue
                    .complete_bootstrap_batch(staging, response);
            }
            PreparedBatchRegisterMutation::Submission(staging, response) => {
                state
                    .registers
                    .queue
                    .complete_submission_batch(staging, response);
            }
        }
        self.write
    }
}

fn prepare_batch_write(
    command: BatchQueueCommand,
    mutation: PreparedBatchRegisterMutation,
) -> Result<PreparedBatchWrite, MmioAccessError> {
    let bytes = command.code().to_le_bytes();
    let (register, value) = decode_write(0x168, &bytes)?;
    Ok(PreparedBatchWrite {
        write: prepare_write(register, value, 0x168, &bytes),
        mutation,
    })
}

pub(super) fn prepare_bootstrap_batch_write(
    staging: BootstrapBatchStaging,
    response: QueueResponseBank,
) -> Result<PreparedBatchWrite, MmioAccessError> {
    prepare_batch_write(
        BatchQueueCommand::Activate,
        PreparedBatchRegisterMutation::Bootstrap(staging, response),
    )
}

pub(super) fn prepare_submission_batch_write(
    staging: SubmissionBatchStaging,
    response: QueueResponseBank,
) -> Result<PreparedBatchWrite, MmioAccessError> {
    prepare_batch_write(
        BatchQueueCommand::PublishSubmissionProducer,
        PreparedBatchRegisterMutation::Submission(staging, response),
    )
}

pub(crate) fn prepare_read(state: &DeviceCore, register: RegisterId, offset: u32) -> PreparedRead {
    PreparedRead {
        response: dispatch::read(state, register).encode(),
        offset,
    }
}

pub(crate) fn prepare_write(
    register: RegisterId,
    value: u64,
    offset: u32,
    request: &[u8],
) -> PreparedWrite {
    PreparedWrite {
        register,
        value,
        offset,
        request: request.to_vec(),
    }
}

#[cfg(test)]
mod tests;
