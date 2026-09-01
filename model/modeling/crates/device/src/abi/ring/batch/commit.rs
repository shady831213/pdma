use super::{
    CommittedBatch, DmaBatchContext, PdmaBatchError, QueueBootstrapBatch, bootstrap_staging,
    command_error, map_access_error, submission_staging, validate_submission_shape,
};
use crate::abi::endpoint::{prepare_bootstrap_batch_write, prepare_submission_batch_write};
use crate::abi::mmio::queue::status_response;
use crate::device::DeviceCore;
use crate::dma::{PdmaMemoryError, PreparedPdmaMapping};
use crate::queue::exact::SubmissionAcceptance;
use hardware_interface_runtime::{MemoryAlignment, ObservationGate};

pub(crate) fn commit_bootstrap(
    state: &mut DeviceCore,
    request: &QueueBootstrapBatch,
    register_gate: &ObservationGate,
) -> Result<CommittedBatch, PdmaBatchError> {
    let regions = request.stage.regions();
    let objects = request.activation.rings();
    if regions.len() != 3
        || regions.iter().map(|region| region.object()).ne([
            objects.submission(),
            objects.completion(),
            objects.event(),
        ])
    {
        return Err(PdmaBatchError::InvalidShape);
    }
    let staging = bootstrap_staging(request.activation, regions)?;
    let context = regions
        .first()
        .copied()
        .map_or(DmaBatchContext::None, DmaBatchContext::Region);
    #[cfg(test)]
    super::reject_injected(state, super::BatchPreparationFault::Capacity, context)?;
    #[cfg(test)]
    super::reject_injected(state, super::BatchPreparationFault::Overlap, context)?;
    let prepared_mapping = state
        .dma
        .prepare_stage(&request.stage)
        .map_err(|error| PdmaBatchError::from_dma(error, context))?;
    let memory = state.dma.backing_memory();
    memory.transaction(|transaction| {
        let mut draft = transaction.try_draft().map_err(|error| {
            PdmaBatchError::from_dma(PdmaMemoryError::from_runtime_memory(&error), context)
        })?;
        draft
            .prepare_regions(prepared_mapping.runtime_regions())
            .map_err(|error| {
                PdmaBatchError::from_dma(PdmaMemoryError::from_runtime_memory(&error), context)
            })?;
        let prepared_queue = state
            .prepare_staged_exact_queue(&request.stage, request.activation)
            .map_err(|error| {
                command_error(error, request.activation.queue().queue_id().raw(), 1)
            })?;
        let response = status_response(prepared_queue.status()).map_err(PdmaBatchError::Command)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Response, context)?;
        let response_bytes = response
            .to_bytes()
            .map(Into::into)
            .map_err(|_| PdmaBatchError::InvalidShape)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Decode, context)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Access, context)?;
        #[cfg(test)]
        super::reject_injected(state, super::BatchPreparationFault::Mask, context)?;
        let command =
            prepare_bootstrap_batch_write(staging, response).map_err(PdmaBatchError::Access)?;
        let reservation = register_gate.reserve(1)?;
        transaction.install_draft(draft);
        state.dma.install_mapping(prepared_mapping);
        state.install_exact_queue_activation(prepared_queue);
        let command = command.commit(state);
        Ok(CommittedBatch {
            response: response_bytes,
            command,
            reservation,
        })
    })
}

pub(crate) fn commit_submission(
    state: &mut DeviceCore,
    request: &SubmissionAcceptance,
    register_gate: &ObservationGate,
) -> Result<CommittedBatch, PdmaBatchError> {
    validate_submission_shape(state, request)?;
    let prepared_mapping = prepare_submission_mapping(state, request)?;
    let staging = submission_staging(request);
    let memory = state.dma.backing_memory();
    memory.transaction(|transaction| {
        let mut draft = transaction.try_draft().map_err(|error| {
            PdmaBatchError::from_dma(
                PdmaMemoryError::from_runtime_memory(&error),
                DmaBatchContext::None,
            )
        })?;
        if let Some(prepared) = prepared_mapping.as_ref() {
            draft
                .prepare_regions(prepared.runtime_regions())
                .map_err(|error| {
                    PdmaBatchError::from_dma(
                        PdmaMemoryError::from_runtime_memory(&error),
                        DmaBatchContext::None,
                    )
                })?;
        }
        if let Some((_, write)) = &request.indirect {
            draft
                .write(
                    crate::dma::runtime_address(write.address()),
                    MemoryAlignment::ONE,
                    write.bytes(),
                )
                .map_err(|error| {
                    map_access_error(PdmaMemoryError::from_runtime_access(&error), write)
                })?;
        }
        draft
            .write(
                crate::dma::runtime_address(request.descriptor.address()),
                MemoryAlignment::ONE,
                request.descriptor.bytes(),
            )
            .map_err(|error| {
                map_access_error(
                    PdmaMemoryError::from_runtime_access(&error),
                    &request.descriptor,
                )
            })?;
        let prepared_cursor = state
            .prepare_submission_publication(request)
            .map_err(|error| command_error(error, request.queue.queue_id().raw(), 3))?;
        let response =
            status_response(prepared_cursor.status()).map_err(PdmaBatchError::Command)?;
        #[cfg(test)]
        super::reject_injected(
            state,
            super::BatchPreparationFault::Response,
            DmaBatchContext::None,
        )?;
        let response_bytes = response
            .to_bytes()
            .map(Into::into)
            .map_err(|_| PdmaBatchError::InvalidShape)?;
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
            prepare_submission_batch_write(staging, response).map_err(PdmaBatchError::Access)?;
        let reservation = register_gate.reserve(1)?;
        transaction.install_draft(draft);
        if let Some(prepared) = prepared_mapping {
            state.dma.install_mapping(prepared);
        }
        state.install_submission_publication(&prepared_cursor);
        let command = command.commit(state);
        Ok(CommittedBatch {
            response: response_bytes,
            command,
            reservation,
        })
    })
}

fn prepare_submission_mapping(
    state: &DeviceCore,
    request: &SubmissionAcceptance,
) -> Result<Option<PreparedPdmaMapping>, PdmaBatchError> {
    let Some((proposal, _)) = &request.indirect else {
        return Ok(None);
    };
    let [region] = proposal.regions() else {
        return Err(PdmaBatchError::InvalidShape);
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
        .dma
        .prepare_stage(proposal)
        .map(Some)
        .map_err(|error| PdmaBatchError::from_dma(error, DmaBatchContext::Region(*region)))
}
