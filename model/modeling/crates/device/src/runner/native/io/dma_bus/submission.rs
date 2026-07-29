mod command;
mod fault;
mod resolve;

use super::super::QueueFetch;
use crate::dma_space::DmaSpace;
use crate::runner::bus::BusControlError;
use crate::runner::bus::control::{BusControlState, transactions};
use pdma_model_contract::abi::{AbiCommand, AbiEvent, DeviceEpoch, EpochQueue, RingRecord};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::{CommandId, CompletionRingId, QueueId};

pub(super) fn fetch(
    dma_space: &DmaSpace,
    control: &BusControlState,
    queue_id: QueueId,
    completion_ring: CompletionRingId,
) -> ModelResult<Option<QueueFetch>> {
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
        .base()
        .checked_add(offset)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let bytes = dma_space
        .read_iova(slot, u64::from(RingRecord::Submission.record_size()))
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let command_id = command_id(&bytes)?;
    match AbiCommand::decode(&bytes) {
        Ok(command) => command::resolve(dma_space, queue_id, completion_ring, command_id, command)
            .map(|result| Some(result.map_or_else(QueueFetch::Fault, QueueFetch::Command))),
        Err(error) => fault::descriptor(slot, &bytes, error).map(|fault| {
            Some(QueueFetch::Fault(AbiEvent::new(
                queue_id, command_id, fault,
            )))
        }),
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
    let raw = pdma_model_contract::abi::read_u64_le(bytes, 8..16)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    usize::try_from(raw)
        .map(CommandId::new)
        .map_err(|_| ModelError::DmaSubmissionState)
}
