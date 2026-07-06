use super::QueuePortDeviceModel;
use super::broadcast;
use super::copy;
use super::resources::ResourceDemand;
use super::retirement;
use super::vector_add;
use crate::runner::AsyncStallReason;
use crate::runner::backend::ModelStepStatus;
use crate::trace::{command_id, operation_from_external};
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;

pub(super) fn step(device: &mut QueuePortDeviceModel) -> ModelResult<ModelStepStatus> {
    if device.pending_retirement.is_some() {
        return retirement::retry_pending_retirement(device);
    }
    let Some(queue_id) = device.runnable.pop_front() else {
        device.trace.idle();
        return Ok(ModelStepStatus::Idle);
    };
    let Some(command) = device.queue(queue_id)?.commands.front().cloned() else {
        return Ok(ModelStepStatus::Progressed);
    };
    let operation = operation_from_external(&command);
    device
        .trace
        .scheduled(queue_id, operation, command_id(&command));
    if !ResourceDemand::for_command(&command).fits(device.resource_config) {
        device.requeue_if_command_pending(queue_id)?;
        device
            .trace
            .stall(Some(queue_id), "ExecutionResourcePressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::ExecutionResourcePressure,
        ));
    }
    match command {
        ExternalCommand::Copy(copy_command) => copy::retire_copy(device, queue_id, copy_command),
        ExternalCommand::StridedCopy(strided) => {
            copy::retire_strided_copy(device, queue_id, strided)
        }
        ExternalCommand::BroadcastCopy(broadcast_command) => {
            broadcast::retire_broadcast_copy(device, queue_id, &broadcast_command)
        }
        ExternalCommand::VectorAdd(vector_add) => {
            vector_add::retire_vector_add(device, queue_id, vector_add)
        }
    }
}
