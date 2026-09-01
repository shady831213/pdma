use super::ActiveQueue;
use crate::abi::ring::PdmaControlError;
use crate::abi::ring::control::transactions;
use crate::adapter::command::ExactCommandContext;
use crate::dma::PdmaMemoryMappings;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{AbiCommand, AbiCommandView, AbiEvent, DescriptorDecodeError};
use pdma_model_contract::semantic::{Command, CommandId, QueueId};

#[derive(Debug, Clone)]
pub(super) enum ExactFetch {
    Command(Command),
    Fault(QueueId, AbiEvent),
}

pub(super) fn fetch(
    dma_space: &PdmaMemoryMappings,
    active: &ActiveQueue,
    queue_id: QueueId,
) -> ModelResult<Option<ExactFetch>> {
    if !active.submission.has_captured_entry() {
        return Ok(None);
    }
    let ring = active.submission.ring();
    let slot = ring
        .entry_address(ring.consumer())
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let bytes = dma_space
        .read_memory(slot, u64::from(ring.layout().entry_size()))
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let (exact_command_id, command_id) = command_id(&bytes)?;
    let exact_queue_id =
        u64::try_from(queue_id.raw()).map_err(|_| ModelError::DmaSubmissionState)?;
    match AbiCommand::decode(&bytes) {
        Ok(command) => ExactCommandContext::new(exact_queue_id, exact_command_id, command_id)
            .resolve(dma_space, command)
            .map(|result| {
                Some(result.map_or_else(
                    |event| ExactFetch::Fault(queue_id, event),
                    ExactFetch::Command,
                ))
            }),
        Err(DescriptorDecodeError::Violation(violation)) => Ok(Some(ExactFetch::Fault(
            queue_id,
            AbiEvent::new(
                exact_queue_id,
                exact_command_id,
                violation.into_abi_fault(slot),
            ),
        ))),
        Err(DescriptorDecodeError::Codec(_) | DescriptorDecodeError::MetadataMismatch) => {
            Err(ModelError::DmaSubmissionState)
        }
    }
}

pub(super) fn consume(active: &mut ActiveQueue) -> Result<(), PdmaControlError> {
    transactions::advance_submission_consumer_raw(active)
}

pub(super) fn has_pending(active: &ActiveQueue) -> bool {
    active.submission.has_captured_entry()
}

fn command_id(bytes: &[u8]) -> ModelResult<(u64, CommandId)> {
    let view = AbiCommandView::new(bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let raw = view
        .header()
        .and_then(|header| header.command_id())
        .map_err(|_| ModelError::DmaSubmissionState)?;
    usize::try_from(raw)
        .map(|model| (raw, CommandId::new(model)))
        .map_err(|_| ModelError::DmaSubmissionState)
}
