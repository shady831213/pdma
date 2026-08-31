use super::{DeviceExecution, SuccessRetirement};
use crate::ModelResult;
use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use pdma_model_contract::semantic::QueueId;

pub(super) fn completion_publication_is_full(
    device: &DeviceExecution,
    control: &QueueControlState,
    success: SuccessRetirement,
) -> ModelResult<bool> {
    Ok(device.queue(success.queue)?.io.completion_blocked(
        control,
        success.queue,
        device.execution.resources().completion_publication_slots(),
    ))
}

pub(super) fn fault_publication_is_full(
    device: &DeviceExecution,
    control: &QueueControlState,
    queue: QueueId,
) -> ModelResult<bool> {
    Ok(device.queue(queue)?.io.fault_blocked(
        control,
        queue,
        device.execution.resources().event_publication_slots(),
    ))
}

pub(super) fn consume_command(
    device: &mut DeviceExecution,
    control: &mut QueueControlState,
    queue: QueueId,
) -> ModelResult<()> {
    device.queue_mut(queue)?.io.consume(control, queue)
}

pub(super) fn retire_command(
    device: &mut DeviceExecution,
    memory: &PdmaMemoryMappings,
    control: &mut QueueControlState,
    queue: QueueId,
) -> ModelResult<()> {
    consume_command(device, control, queue)?;
    device.requeue_if_command_pending(memory, control, queue)
}
