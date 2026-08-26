mod command;
mod resolve;

use crate::dma_space::PdmaMemoryMappings;
use crate::runner::bus::BusControlError;
use crate::runner::bus::control::{BusControlState, transactions};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiCommand, DescriptorDecodeError, DescriptorSchemaView, RingRecord,
};
use pdma_model_contract::bridge::AbiEvent;
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueId};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue};

#[derive(Debug, Clone)]
pub(super) enum ExactFetch {
    Command(crate::runner::native::execution::ExecutionCommand),
    Fault(AbiEvent),
}

pub(super) fn fetch(
    dma_space: &PdmaMemoryMappings,
    control: &BusControlState,
    queue_id: QueueId,
    completion_ring: CompletionRingId,
) -> ModelResult<Option<ExactFetch>> {
    let queue = EpochQueue::new(DeviceEpoch::new(control.epoch().raw()), queue_id);
    let active = control
        .queue(queue)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    if active.submission.consumer() >= active.captured_submission_producer {
        return Ok(None);
    }
    let layout = active.activation.rings().submission();
    let offset = layout
        .slot(active.submission.consumer())
        .checked_mul(u64::from(RingRecord::Submission.record_size()))
        .ok_or(ModelError::DmaSubmissionState)?;
    let slot = layout
        .region()
        .abi_base()
        .checked_add(offset)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let bytes = dma_space
        .read_memory(slot, u64::from(RingRecord::Submission.record_size()))
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let command_id = command_id(&bytes)?;
    match AbiCommand::decode(&bytes) {
        Ok(command) => command::resolve(
            dma_space,
            command::ExactCommandContext::new(queue_id, completion_ring, command_id),
            command,
        )
        .map(|result| Some(result.map_or_else(ExactFetch::Fault, ExactFetch::Command))),
        Err(DescriptorDecodeError::Violation(violation)) => Ok(Some(ExactFetch::Fault(
            AbiEvent::new(queue_id, command_id, violation.into_abi_fault(slot)),
        ))),
        Err(DescriptorDecodeError::Codec(_) | DescriptorDecodeError::MetadataMismatch) => {
            Err(ModelError::DmaSubmissionState)
        }
    }
}

pub(super) fn consume(
    control: &mut BusControlState,
    queue_id: QueueId,
) -> Result<(), BusControlError> {
    transactions::advance_submission_consumer_raw(control, queue_id)
}

pub(super) fn has_pending(control: &BusControlState, queue_id: QueueId) -> bool {
    let queue = EpochQueue::new(control.epoch(), queue_id);
    control
        .queue(queue)
        .is_ok_and(|active| active.submission.consumer() < active.captured_submission_producer)
}

fn command_id(bytes: &[u8]) -> ModelResult<CommandId> {
    let view = DescriptorSchemaView::new(bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let raw = view
        .header()
        .and_then(|header| header.command_id())
        .map_err(|_| ModelError::DmaSubmissionState)?;
    usize::try_from(raw)
        .map(CommandId::new)
        .map_err(|_| ModelError::DmaSubmissionState)
}
