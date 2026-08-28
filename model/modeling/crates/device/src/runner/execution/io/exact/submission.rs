mod command;
mod resolve;

use crate::dma_space::PdmaMemoryMappings;
use crate::runner::abi::bus::PdmaControlError;
use crate::runner::abi::bus::control::{BusControlState, transactions};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandView, DescriptorDecodeError, ExactAbiEvent, RingRecord,
};
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueId};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue};

#[derive(Debug, Clone)]
pub(super) enum ExactFetch {
    Command(crate::runner::execution::command::ExecutionCommand),
    Fault(QueueId, ExactAbiEvent),
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
    let (exact_command_id, command_id) = command_id(&bytes)?;
    let exact_queue_id =
        u64::try_from(queue_id.raw()).map_err(|_| ModelError::DmaSubmissionState)?;
    match AbiCommand::decode(&bytes) {
        Ok(command) => command::resolve(
            dma_space,
            command::ExactCommandContext::new(
                exact_queue_id,
                completion_ring,
                exact_command_id,
                command_id,
            ),
            command,
        )
        .map(|result| {
            Some(result.map_or_else(
                |event| ExactFetch::Fault(queue_id, event),
                ExactFetch::Command,
            ))
        }),
        Err(DescriptorDecodeError::Violation(violation)) => Ok(Some(ExactFetch::Fault(
            queue_id,
            ExactAbiEvent::new(
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

pub(super) fn consume(
    control: &mut BusControlState,
    queue_id: QueueId,
) -> Result<(), PdmaControlError> {
    transactions::advance_submission_consumer_raw(control, queue_id)
}

pub(super) fn has_pending(control: &BusControlState, queue_id: QueueId) -> bool {
    let queue = EpochQueue::new(control.epoch(), queue_id);
    control
        .queue(queue)
        .is_ok_and(|active| active.submission.consumer() < active.captured_submission_producer)
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
