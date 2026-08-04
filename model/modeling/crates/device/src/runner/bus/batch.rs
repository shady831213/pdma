use super::{AsyncDeviceHandle, BusControlError, SubmissionAcceptance};
use crate::dma_space::DmaSpaceError;
use crate::runner::bus::control::{activation, transactions};
use crate::runner::mmio::queue::{BootstrapBatchStaging, SubmissionBatchStaging, status_response};
use crate::runner::state::{publish, wake};
use pdma_model_contract::abi::QueueActivation;

#[path = "batch/error.rs"]
mod error;
pub use error::{DmaBatchContext, MmioBatchError};

#[cfg(test)]
#[path = "batch/tests.rs"]
mod tests;

/// Exact raw queue response-bank bytes.
pub type QueueResponseBytes = [u8; 72];

/// The one closed queue-bootstrap batch request.
#[derive(Debug)]
pub struct QueueBootstrapBatch {
    stage: crate::DmaStage,
    activation: QueueActivation,
}

impl QueueBootstrapBatch {
    /// Creates the fixed stage, queue-staging, `ACTIVATE`, response-read grammar.
    pub const fn new(stage: crate::DmaStage, activation: QueueActivation) -> Self {
        Self { stage, activation }
    }
}

/// The one closed submission batch request.
#[derive(Debug)]
pub struct SubmissionBatch(SubmissionAcceptance);

impl SubmissionBatch {
    /// Creates the fixed optional-indirect, descriptor, publication, response-read grammar.
    pub const fn new(
        queue: pdma_model_contract::abi::EpochQueue,
        expected_producer: pdma_model_contract::abi::RingCursor,
        descriptor: super::DmaWrite,
        indirect: Option<(crate::DmaStage, super::DmaWrite)>,
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
        let (response, generation) = {
            let mut state = self.shared.state.lock().await;
            let response = prepare_bootstrap(&mut state, &request)?;
            wake(&mut state);
            (response, state.generation)
        };
        publish(&self.shared, generation);
        Ok(response)
    }

    /// Applies one closed submission batch under one lock after complete projection.
    pub async fn submit_queue_batch(
        &self,
        request: SubmissionBatch,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        let (response, generation) = {
            let mut state = self.shared.state.lock().await;
            let response = prepare_submission(&mut state, &request.0)?;
            wake(&mut state);
            (response, state.generation)
        };
        publish(&self.shared, generation);
        Ok(response)
    }
}

fn prepare_bootstrap(
    state: &mut crate::runner::state::RunnerState,
    request: &QueueBootstrapBatch,
) -> Result<QueueResponseBytes, MmioBatchError> {
    let regions = request.stage.regions();
    let rings = request.activation.rings();
    let expected = [
        rings.submission().region(),
        rings.completion().region(),
        rings.event().region(),
    ];
    if regions != expected {
        return Err(MmioBatchError::InvalidShape);
    }
    let staging = bootstrap_staging(request.activation)?;
    let context = regions
        .first()
        .copied()
        .map_or(DmaBatchContext::None, DmaBatchContext::Region);
    let mut prepared_dma = state
        .dma_space
        .prepare_clone()
        .map_err(|error| MmioBatchError::from_dma(error, DmaBatchContext::None))?;
    prepared_dma
        .commit(request.stage.clone())
        .map_err(|error| MmioBatchError::from_dma(error, context))?;
    let prepared_queue = activation::prepare_staged(state, &request.stage, request.activation)
        .map_err(|error| command_error(error, request.activation.queue().queue_id().raw(), 1))?;
    let status = prepared_queue.status();
    let response = status_response(status).map_err(MmioBatchError::Command)?;
    state.dma_space.install_prepared(prepared_dma);
    activation::install(state, prepared_queue);
    state.mmio.queue.complete_bootstrap_batch(staging, response);
    Ok(response.to_le_bytes())
}

fn prepare_submission(
    state: &mut crate::runner::state::RunnerState,
    request: &SubmissionAcceptance,
) -> Result<QueueResponseBytes, MmioBatchError> {
    validate_submission_shape(state, request)?;
    let mut prepared_dma = state
        .dma_space
        .prepare_clone()
        .map_err(|error| MmioBatchError::from_dma(error, DmaBatchContext::None))?;
    if let Some((stage, write)) = &request.indirect {
        let [region] = stage.regions() else {
            return Err(MmioBatchError::InvalidShape);
        };
        let region = *region;
        prepared_dma
            .commit(stage.clone())
            .map_err(|error| MmioBatchError::from_dma(error, DmaBatchContext::Region(region)))?;
        prepared_dma
            .write_iova(write.address(), write.bytes())
            .map_err(|error| map_access_error(error, write))?;
    }
    prepared_dma
        .write_iova(request.descriptor.address(), request.descriptor.bytes())
        .map_err(|error| map_access_error(error, &request.descriptor))?;
    let prepared_cursor = transactions::prepare_submission_publication(state, request)
        .map_err(|error| command_error(error, request.queue.queue_id().raw(), 3))?;
    let status = prepared_cursor.status();
    let response = status_response(status).map_err(MmioBatchError::Command)?;
    let staging = submission_staging(request);
    state.dma_space.install_prepared(prepared_dma);
    transactions::install_submission_publication(state, &prepared_cursor);
    state
        .mmio
        .queue
        .complete_submission_batch(staging, response);
    Ok(response.to_le_bytes())
}

fn command_error(error: BusControlError, queue_id: usize, command: u32) -> MmioBatchError {
    let numeric_id = u64::try_from(queue_id).unwrap_or(u64::MAX);
    MmioBatchError::Command(crate::runner::mmio::queue::map_bus_error(
        error, numeric_id, command, 0,
    ))
}

fn map_access_error(error: DmaSpaceError, write: &super::DmaWrite) -> MmioBatchError {
    MmioBatchError::from_dma(
        error,
        DmaBatchContext::Access {
            address: write.address(),
            length: u64::try_from(write.bytes().len()).unwrap_or(u64::MAX),
        },
    )
}

fn validate_submission_shape(
    _state: &crate::runner::state::RunnerState,
    request: &SubmissionAcceptance,
) -> Result<(), MmioBatchError> {
    if request.descriptor.bytes().len() != 64 {
        return Err(MmioBatchError::InvalidShape);
    }
    if let Some((stage, write)) = &request.indirect {
        let [region] = stage.regions() else {
            return Err(MmioBatchError::InvalidShape);
        };
        if write.address() != region.base()
            || u64::try_from(write.bytes().len()) != Ok(region.length())
        {
            return Err(MmioBatchError::InvalidShape);
        }
    }
    Ok(())
}

fn bootstrap_staging(activation: QueueActivation) -> Result<BootstrapBatchStaging, MmioBatchError> {
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

fn submission_staging(request: &SubmissionAcceptance) -> SubmissionBatchStaging {
    SubmissionBatchStaging {
        expected_epoch: request.queue.epoch().raw(),
        id: u64::try_from(request.queue.queue_id().raw()).unwrap_or(u64::MAX),
        expected_cursor: request.expected_producer.raw(),
        new_cursor: request.expected_producer.raw().saturating_add(1),
    }
}
