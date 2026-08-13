use super::NativeExecution;
use super::broadcast;
use super::copy;
use super::io::NormalizedFetch;
use super::probe::{RunStateProbe, probe_run_state};
use super::retirement;
use super::vector_add;
use crate::ModelResult;
use crate::runner::AsyncStallReason;
use crate::runner::backend::ModelStepStatus;
use crate::trace::{command_id, operation_from_external};
use pdma_model_contract::desc::ExternalCommand;

pub(super) fn step(execution: NativeExecution<'_>) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let blocked_resource = match probe_run_state(device, dma_space, bus_control) {
        RunStateProbe::Idle => {
            device.trace.idle();
            return Ok(ModelStepStatus::Idle);
        }
        RunStateProbe::Running => None,
        RunStateProbe::Blocked {
            reason: AsyncStallReason::ExecutionResourcePressure,
            ..
        } => Some(AsyncStallReason::ExecutionResourcePressure),
        RunStateProbe::Blocked { queue, reason } => {
            device.trace.stall(Some(queue), stall_name(reason));
            return Ok(ModelStepStatus::Stalled(reason));
        }
    };
    if device.pending_retirement.is_some() {
        return retirement::retry_pending_retirement(device, dma_space, bus_control);
    }
    let Some(queue_id) = device.runnable.pop_front() else {
        return Ok(ModelStepStatus::Idle);
    };
    let entry = device.queue(queue_id)?.entry;
    let Some(fetched) =
        device
            .queue_mut(queue_id)?
            .frontend
            .fetch(dma_space, bus_control, entry)?
    else {
        return Ok(ModelStepStatus::Progressed);
    };
    device.native_steps += 1;
    let command = match fetched {
        NormalizedFetch::Command(command) => command,
        NormalizedFetch::Fault(fault) => {
            return retirement::cache_frontend_fault(device, bus_control, fault);
        }
    };
    let operation = operation_from_external(&command);
    device
        .trace
        .scheduled(queue_id, operation, command_id(&command));
    if let Some(reason) = blocked_resource {
        device.requeue_if_command_pending(dma_space, bus_control, queue_id)?;
        device.trace.stall(Some(queue_id), stall_name(reason));
        return Ok(ModelStepStatus::Stalled(reason));
    }
    match command {
        ExternalCommand::Copy(copy_command) => copy::retire_copy(
            NativeExecution {
                device,
                dma_space,
                bus_control,
            },
            queue_id,
            copy_command,
        ),
        ExternalCommand::StridedCopy(strided) => copy::retire_strided_copy(
            NativeExecution {
                device,
                dma_space,
                bus_control,
            },
            queue_id,
            strided,
        ),
        ExternalCommand::BroadcastCopy(broadcast_command) => broadcast::retire_broadcast_copy(
            NativeExecution {
                device,
                dma_space,
                bus_control,
            },
            queue_id,
            &broadcast_command,
        ),
        ExternalCommand::VectorAdd(vector_add) => vector_add::retire_vector_add(
            NativeExecution {
                device,
                dma_space,
                bus_control,
            },
            queue_id,
            vector_add,
        ),
    }
}

const fn stall_name(reason: AsyncStallReason) -> &'static str {
    match reason {
        AsyncStallReason::ExecutionResourcePressure => "ExecutionResourcePressure",
        AsyncStallReason::CompletionPublicationPressure => "CompletionPublicationPressure",
        AsyncStallReason::EventPublicationPressure => "EventPublicationPressure",
        AsyncStallReason::FaultRetirementPendingRetry => "FaultRetirementPendingRetry",
    }
}
