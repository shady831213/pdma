use super::NativeDevice;
use super::broadcast;
use super::copy;
use super::resources::ResourceDemand;
use super::retirement;
use super::vector_add;
use crate::runner::backend::BackendStepStatus;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;

pub(super) fn step(device: &mut NativeDevice) -> ModelResult<BackendStepStatus> {
    if device.pending_retirement.is_some() {
        return retirement::retry_pending_retirement(device);
    }
    let Some(queue_id) = device.runnable.pop_front() else {
        return Ok(BackendStepStatus::Idle);
    };
    let Some(command) = device.queue(queue_id)?.commands.front().cloned() else {
        return Ok(BackendStepStatus::Idle);
    };
    if !ResourceDemand::for_command(&command).fits(device.resource_config) {
        device.requeue_if_command_pending(queue_id)?;
        return Ok(BackendStepStatus::Stalled);
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
