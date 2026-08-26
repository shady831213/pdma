use super::{AsyncDeviceHandle, BusControlError, SubmissionAcceptance};
use crate::runner::abi::mmio::queue::{BootstrapBatchStaging, SubmissionBatchStaging};
use crate::runner::abi::register_endpoint::PreparedWrite;
use crate::runner::state::publish;
use hardware_interface_runtime::{AccessOrigin, ObservationReservation};
use pdma_model_contract::binding::QueueActivation;

mod error;
pub use error::{DmaBatchContext, MmioBatchError, PdmaBatchError};
mod commit;
use commit::{commit_bootstrap, commit_submission};

#[cfg(test)]
mod tests;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runner) enum BatchPreparationFault {
    Capacity,
    Overlap,
    Decode,
    Access,
    Mask,
    Response,
}

#[cfg(test)]
pub(super) fn reject_injected(
    state: &crate::runner::state::DeviceCore,
    expected: BatchPreparationFault,
    context: DmaBatchContext,
) -> Result<(), MmioBatchError> {
    if state.batch_fault.get() != Some(expected) {
        return Ok(());
    }
    state.batch_fault.set(None);
    let error = match expected {
        BatchPreparationFault::Capacity => {
            MmioBatchError::from_dma(crate::PdmaMemoryError::StorageExhausted, context)
        }
        BatchPreparationFault::Overlap => {
            MmioBatchError::from_dma(crate::PdmaMemoryError::OverlappingRegion, context)
        }
        BatchPreparationFault::Decode => MmioBatchError::Access(
            pdma_model_contract::abi::mmio::MmioAccessError::ReservedOffset { offset: 0x168 },
        ),
        BatchPreparationFault::Access => MmioBatchError::Access(
            pdma_model_contract::abi::mmio::MmioAccessError::WriteToReadOnly { offset: 0x168 },
        ),
        BatchPreparationFault::Mask => MmioBatchError::Access(
            pdma_model_contract::abi::mmio::MmioAccessError::ReservedBits {
                offset: 0x168,
                bits: 4,
            },
        ),
        BatchPreparationFault::Response => MmioBatchError::InvalidShape,
    };
    Err(error)
}

/// Exact raw queue response-bank bytes.
pub type QueueResponseBytes = [u8; 72];

/// The one closed queue-bootstrap batch request.
#[derive(Debug)]
pub struct QueueBootstrapBatch {
    stage: crate::PdmaMemoryStage,
    activation: QueueActivation,
}

impl QueueBootstrapBatch {
    /// Creates the fixed stage, queue-staging, `ACTIVATE`, response-read grammar.
    pub const fn new(stage: crate::PdmaMemoryStage, activation: QueueActivation) -> Self {
        Self { stage, activation }
    }
}

/// The one closed submission batch request.
#[derive(Debug)]
pub struct SubmissionBatch(SubmissionAcceptance);

impl SubmissionBatch {
    /// Creates the fixed optional-indirect, descriptor, publication, response-read grammar.
    pub const fn new(
        queue: pdma_model_contract::semantic::EpochQueue,
        expected_producer: pdma_model_contract::abi::RingCursor,
        descriptor: super::DmaWrite,
        indirect: Option<(crate::PdmaMemoryStage, super::DmaWrite)>,
    ) -> Self {
        Self(SubmissionAcceptance::new(
            queue,
            expected_producer,
            descriptor,
            indirect,
        ))
    }
}

impl AsyncDeviceHandle {
    /// Applies one closed bootstrap batch under one lock after complete projection.
    pub async fn bootstrap_queue_batch(
        &self,
        request: QueueBootstrapBatch,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        let committed = {
            let mut state = self.shared.state.lock().await;
            commit_bootstrap(&mut state, &request)?
        };
        committed.emit(&self.shared)
    }

    /// Applies one closed submission batch under one lock after complete projection.
    pub async fn submit_queue_batch(
        &self,
        request: SubmissionBatch,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        let committed = {
            let mut state = self.shared.state.lock().await;
            commit_submission(&mut state, &request.0)?
        };
        committed.emit(&self.shared)
    }
}

pub(super) struct CommittedBatch {
    response: QueueResponseBytes,
    command: PreparedWrite,
    reservation: ObservationReservation,
    generation: usize,
}

impl CommittedBatch {
    fn emit(
        self,
        shared: &crate::runner::state::RunnerShared,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        let observation = self
            .command
            .observation(self.reservation.start_sequence(), AccessOrigin::Batch);
        self.reservation.emit(vec![observation])?;
        publish(shared, self.generation);
        Ok(self.response)
    }
}

pub(super) fn command_error(
    error: BusControlError,
    queue_id: usize,
    command: u32,
) -> MmioBatchError {
    let numeric_id = u64::try_from(queue_id).unwrap_or(u64::MAX);
    MmioBatchError::Command(crate::runner::abi::mmio::queue::map_bus_error(
        error, numeric_id, command, 0,
    ))
}

pub(super) fn map_access_error(
    error: crate::PdmaMemoryError,
    write: &super::DmaWrite,
) -> MmioBatchError {
    MmioBatchError::from_dma(
        error,
        DmaBatchContext::Access {
            address: write.address(),
            length: u64::try_from(write.bytes().len()).unwrap_or(u64::MAX),
        },
    )
}

pub(super) fn validate_submission_shape(
    _state: &crate::runner::state::DeviceCore,
    request: &SubmissionAcceptance,
) -> Result<(), MmioBatchError> {
    if request.descriptor.bytes().len() != 64 {
        return Err(MmioBatchError::InvalidShape);
    }
    if let Some((stage, write)) = &request.indirect {
        let [region] = stage.regions() else {
            return Err(MmioBatchError::InvalidShape);
        };
        if write.address() != region.abi_base()
            || u64::try_from(write.bytes().len()) != Ok(region.length())
        {
            return Err(MmioBatchError::InvalidShape);
        }
    }
    Ok(())
}

pub(super) fn bootstrap_staging(
    activation: QueueActivation,
) -> Result<BootstrapBatchStaging, MmioBatchError> {
    let rings = activation.rings();
    Ok(BootstrapBatchStaging {
        expected_epoch: activation.queue().epoch().raw(),
        id: u64::try_from(activation.queue().queue_id().raw())
            .map_err(|_| MmioBatchError::InvalidShape)?,
        cfg_software_owner: u64::try_from(activation.config().software_owner().raw())
            .map_err(|_| MmioBatchError::InvalidShape)?,
        sq_base: rings.submission().region().base().raw(),
        sq_depth: rings.submission().depth(),
        cq_base: rings.completion().region().base().raw(),
        cq_depth: rings.completion().depth(),
        eq_base: rings.event().region().base().raw(),
        eq_depth: rings.event().depth(),
    })
}

pub(super) fn submission_staging(request: &SubmissionAcceptance) -> SubmissionBatchStaging {
    SubmissionBatchStaging {
        expected_epoch: request.queue.epoch().raw(),
        id: u64::try_from(request.queue.queue_id().raw()).unwrap_or(u64::MAX),
        expected_cursor: request.expected_producer.raw(),
        new_cursor: request.expected_producer.raw().saturating_add(1),
    }
}
