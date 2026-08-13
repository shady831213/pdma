use super::{
    CommittedBatch, DmaBatchContext, MmioBatchError, QueueBootstrapBatch, bootstrap_staging,
    command_error, map_access_error, submission_staging, validate_submission_shape,
};
use crate::dma_space::{PdmaMemoryError, PreparedPdmaMapping};
use crate::runner::bus::SubmissionAcceptance;
use crate::runner::bus::control::{activation, transactions};
use crate::runner::mmio::queue::status_response;
use crate::runner::register_endpoint::{
    prepare_bootstrap_batch_write, prepare_submission_batch_write,
};
use crate::runner::state::{DeviceCore, wake};
use hardware_interface_runtime::MemoryAlignment;

pub(super) fn commit_bootstrap(
    state: &mut DeviceCore,
    request: &QueueBootstrapBatch,
) -> Result<CommittedBatch, MmioBatchError> {
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
    #[cfg(test)]
    super::reject_injected(state, super::BatchPreparationFault::Capacity, context)?;
    #[cfg(test)]
    super::reject_injected(state, super::BatchPreparationFault::Overlap, context)?;
    let prepared_mapping = state
        .dma_space
        .prepare_stage(&request.stage)
        .map_err(|error| MmioBatchError::from_dma(error, context))?;
    let memory = state.dma_space.backing_memory();
    memory.transaction(|transaction| {
        let mut draft = transaction.try_draft().map_err(|error| {
            MmioBatchError::from_dma(PdmaMemoryError::from_runtime_memory(&error), context)
        })?;
        draft
            .prepare_regions(prepared_mapping.runtime_regions())
            .map_err(|error| {
                MmioBatchError::from_dma(PdmaMemoryError::from_runtime_memory(&error), context)
            })?;
        let prepared_queue = activation::prepare_staged(state, &request.stage, request.activation)
            .map_err(|error| {
                command_error(error, request.activation.queue().queue_id().raw(), 1)
            })?;
        let response = status_response(prepared_queue.status()).map_err(MmioBatchError::Command)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Response, context)?;
        let response_bytes = response
            .to_le_bytes()
            .map_err(|_| MmioBatchError::InvalidShape)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Decode, context)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Access, context)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Mask, context)?;
        let command =
            prepare_bootstrap_batch_write(staging, response).map_err(MmioBatchError::Access)?;
        let reservation = state.register_gate.reserve(1)?;
        transaction.install_draft(draft);
        state.dma_space.install_mapping(prepared_mapping);
        activation::install(state, prepared_queue);
        let command = command.commit(state);
        wake(state);
        Ok(CommittedBatch {
            response: response_bytes,
            command,
            reservation,
            generation: state.generation,
        })
    })
}

pub(super) fn commit_submission(
    state: &mut DeviceCore,
    request: &SubmissionAcceptance,
) -> Result<CommittedBatch, MmioBatchError> {
    validate_submission_shape(state, request)?;
    let prepared_mapping = prepare_submission_mapping(state, request)?;
    let staging = submission_staging(request);
    let memory = state.dma_space.backing_memory();
    memory.transaction(|transaction| {
        let mut draft = transaction.try_draft().map_err(|error| {
            MmioBatchError::from_dma(
                PdmaMemoryError::from_runtime_memory(&error),
                DmaBatchContext::None,
            )
        })?;
        if let Some(prepared) = prepared_mapping.as_ref() {
            draft
                .prepare_regions(prepared.runtime_regions())
                .map_err(|error| {
                    MmioBatchError::from_dma(
                        PdmaMemoryError::from_runtime_memory(&error),
                        DmaBatchContext::None,
                    )
                })?;
        }
        if let Some((_, write)) = &request.indirect {
            draft
                .write(write.address().into(), MemoryAlignment::ONE, write.bytes())
                .map_err(|error| {
                    map_access_error(PdmaMemoryError::from_runtime_access(&error), write)
                })?;
        }
        draft
            .write(
                request.descriptor.address().into(),
                MemoryAlignment::ONE,
                request.descriptor.bytes(),
            )
            .map_err(|error| {
                map_access_error(
                    PdmaMemoryError::from_runtime_access(&error),
                    &request.descriptor,
                )
            })?;
        let prepared_cursor = transactions::prepare_submission_publication(state, request)
            .map_err(|error| command_error(error, request.queue.queue_id().raw(), 3))?;
        let response =
            status_response(prepared_cursor.status()).map_err(MmioBatchError::Command)?;
        #[cfg(test)]
        super::reject_injected(
            state,
            super::BatchPreparationFault::Response,
            DmaBatchContext::None,
        )?;
        let response_bytes = response
            .to_le_bytes()
            .map_err(|_| MmioBatchError::InvalidShape)?;
        #[cfg(test)]
        super::reject_injected(
            state,
            super::BatchPreparationFault::Decode,
            DmaBatchContext::None,
        )?;
        #[cfg(test)]
        super::reject_injected(
            state,
            super::BatchPreparationFault::Access,
            DmaBatchContext::None,
        )?;
        #[cfg(test)]
        super::reject_injected(
            state,
            super::BatchPreparationFault::Mask,
            DmaBatchContext::None,
        )?;
        let command =
            prepare_submission_batch_write(staging, response).map_err(MmioBatchError::Access)?;
        let reservation = state.register_gate.reserve(1)?;
        transaction.install_draft(draft);
        if let Some(prepared) = prepared_mapping {
            state.dma_space.install_mapping(prepared);
        }
        transactions::install_submission_publication(state, &prepared_cursor);
        let command = command.commit(state);
        wake(state);
        Ok(CommittedBatch {
            response: response_bytes,
            command,
            reservation,
            generation: state.generation,
        })
    })
}

fn prepare_submission_mapping(
    state: &DeviceCore,
    request: &SubmissionAcceptance,
) -> Result<Option<PreparedPdmaMapping>, MmioBatchError> {
    let Some((proposal, _)) = &request.indirect else {
        return Ok(None);
    };
    let [region] = proposal.regions() else {
        return Err(MmioBatchError::InvalidShape);
    };
    #[cfg(test)]
    super::reject_injected(
        state,
        super::BatchPreparationFault::Capacity,
        DmaBatchContext::Region(*region),
    )?;
    #[cfg(test)]
    super::reject_injected(
        state,
        super::BatchPreparationFault::Overlap,
        DmaBatchContext::Region(*region),
    )?;
    state
        .dma_space
        .prepare_stage(proposal)
        .map(Some)
        .map_err(|error| MmioBatchError::from_dma(error, DmaBatchContext::Region(*region)))
}
