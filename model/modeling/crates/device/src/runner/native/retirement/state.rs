use super::{QueuePortDeviceModel, SuccessRetirement};
use crate::ModelResult;
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::bus::control::BusControlState;
use pdma_model_contract::semantic::QueueId;

pub(super) fn completion_publication_is_full(
    device: &QueuePortDeviceModel,
    control: &BusControlState,
    success: SuccessRetirement,
) -> ModelResult<bool> {
    Ok(device.queue(success.queue)?.frontend.completion_blocked(
        control,
        success.queue,
        device.resource_config.completion_publication_slots(),
    ))
}

pub(super) fn fault_publication_is_full(
    device: &QueuePortDeviceModel,
    control: &BusControlState,
    queue: QueueId,
) -> ModelResult<bool> {
    Ok(device.queue(queue)?.frontend.fault_blocked(
        control,
        queue,
        device.resource_config.event_publication_slots(),
    ))
}

pub(super) fn consume_command(
    device: &mut QueuePortDeviceModel,
    control: &mut BusControlState,
    queue: QueueId,
) -> ModelResult<()> {
    device.queue_mut(queue)?.frontend.consume(control, queue)
}

pub(super) fn retire_command(
    device: &mut QueuePortDeviceModel,
    memory: &PdmaMemoryMappings,
    control: &mut BusControlState,
    queue: QueueId,
) -> ModelResult<()> {
    consume_command(device, control, queue)?;
    device.requeue_if_command_pending(memory, control, queue)
}
