use super::mmio::dispatch;
use super::mmio::queue::{BootstrapBatchStaging, SubmissionBatchStaging};
use super::state::{RunnerShared, publish};
use super::{AsyncDevice, DeviceCore};
use hardware_interface_runtime::{
    AccessOrigin, ObservationGateError, RegisterAccess, RegisterObservation,
};
use pdma_model_contract::abi::mmio::QueueResponseBank;
use pdma_model_contract::abi::mmio::{MmioAccessError, RegisterId, decode_read, decode_write};
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

mod claim;

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

/// Cloneable exact register endpoint over one device core.
#[derive(Clone, Debug)]
pub struct MmioRegisterEndpoint {
    shared: Arc<RunnerShared>,
}

impl MmioRegisterEndpoint {
    /// Consumes a core into a standalone exact register endpoint.
    pub fn from_core(core: DeviceCore) -> Self {
        let (device, _worker) = AsyncDevice::from_core(core);
        device.handle().register_endpoint()
    }

    pub(super) const fn new(shared: Arc<RunnerShared>) -> Self {
        Self { shared }
    }

    /// Claims one exact IRQ notification and emits its complete direct transcript atomically.
    pub async fn claim_notification_exact(
        &self,
    ) -> Result<Option<pdma_model_contract::semantic::IrqNotification>, MmioRegisterError> {
        let committed = {
            let mut state = self.shared.state.lock().await;
            claim::prepare_claim(&mut state)?
        };
        let generation = committed.generation();
        let notification = committed.emit()?;
        if let Some(generation) = generation {
            publish(&self.shared, generation);
        }
        Ok(notification)
    }

    /// Claims one IRQ notification and returns the exact emitted direct transcript.
    pub async fn claim_notification_exact_observed(
        &self,
    ) -> Result<
        (
            Option<pdma_model_contract::semantic::IrqNotification>,
            Vec<RegisterObservation>,
        ),
        MmioRegisterError,
    > {
        let committed = {
            let mut state = self.shared.state.lock().await;
            claim::prepare_claim(&mut state)?
        };
        let generation = committed.generation();
        let result = committed.emit_observed()?;
        if let Some(generation) = generation {
            publish(&self.shared, generation);
        }
        Ok(result)
    }
}

impl RegisterAccess for MmioRegisterEndpoint {
    type Error = MmioRegisterError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        let register = decode_read(offset, response.len() as u128)?;
        let (prepared, reservation) = {
            let state = self.shared.state.lock().await;
            let prepared = prepare_read(&state, register, offset);
            let reservation = state.register_gate.reserve(1)?;
            drop(state);
            (prepared, reservation)
        };
        let observed = prepared.observation(reservation.start_sequence());
        response.copy_from_slice(&prepared.response);
        reservation.emit(vec![observed])?;
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        let (register, value) = decode_write(offset, request)?;
        let (prepared, reservation, generation) = {
            let mut state = self.shared.state.lock().await;
            let prepared = prepare_write(register, value, offset, request);
            let reservation = state.register_gate.reserve(1)?;
            let previous = state.generation;
            prepared.commit(&mut state);
            let generation = (state.generation != previous).then_some(state.generation);
            drop(state);
            (prepared, reservation, generation)
        };
        let observed = prepared.observation(reservation.start_sequence(), AccessOrigin::Direct);
        reservation.emit(vec![observed])?;
        if let Some(generation) = generation {
            publish(&self.shared, generation);
        }
        Ok(())
    }
}

pub(super) struct PreparedRead {
    pub(super) response: Vec<u8>,
    offset: u32,
}

impl PreparedRead {
    fn observation(&self, sequence: u64) -> RegisterObservation {
        RegisterObservation::read(sequence, AccessOrigin::Direct, self.offset, &self.response)
    }
}

#[derive(Clone)]
pub(super) struct PreparedWrite {
    register: RegisterId,
    value: u64,
    offset: u32,
    request: Vec<u8>,
}

impl PreparedWrite {
    pub(super) fn commit(&self, state: &mut DeviceCore) {
        dispatch::write(state, self.register, self.value);
    }

    pub(super) fn observation(&self, sequence: u64, origin: AccessOrigin) -> RegisterObservation {
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
                state.mmio.queue.complete_bootstrap_batch(staging, response);
            }
            PreparedBatchRegisterMutation::Submission(staging, response) => {
                state
                    .mmio
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

pub(super) fn prepare_read(state: &DeviceCore, register: RegisterId, offset: u32) -> PreparedRead {
    PreparedRead {
        response: dispatch::read(state, register).encode(),
        offset,
    }
}

pub(super) fn prepare_write(
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
#[path = "register_endpoint/tests.rs"]
mod tests;
