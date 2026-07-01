use super::NativeDevice;
use crate::runner::backend::BackendStepStatus;
use pdma_model_contract::completion::{CompletionRecord, CompletionStatus};
use pdma_model_contract::desc::{CopyCommand, ExternalCommand};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::{EventErrorKind, EventRecord};
use pdma_model_contract::ids::{BufferId, QueueId};

#[derive(Debug, Clone, Copy)]
struct FaultRetirement {
    queue_id: QueueId,
    command: CopyCommand,
    buffer: BufferId,
    source: ModelError,
}

pub(super) fn step(device: &mut NativeDevice) -> ModelResult<BackendStepStatus> {
    let Some(queue_id) = device.runnable.pop_front() else {
        return Ok(BackendStepStatus::Idle);
    };
    let Some(command) = device.queue(queue_id)?.commands.front().cloned() else {
        return Ok(BackendStepStatus::Progressed);
    };
    match command {
        ExternalCommand::Copy(copy) => retire_copy(device, queue_id, copy),
        ExternalCommand::StridedCopy(_)
        | ExternalCommand::BroadcastCopy(_)
        | ExternalCommand::VectorAdd(_) => {
            let _retired = device.queue_mut(queue_id)?.commands.pop();
            Err(ModelError::StepBudgetExhausted { max_steps: 0 })
        }
    }
}

fn retire_copy(
    device: &mut NativeDevice,
    queue_id: QueueId,
    command: CopyCommand,
) -> ModelResult<BackendStepStatus> {
    let request = command.request();
    let bytes = match device.memory.read_checked(request.src(), 0, request.len()) {
        Ok(bytes) => bytes,
        Err(source) => {
            return publish_fault(
                device,
                FaultRetirement {
                    queue_id,
                    command,
                    buffer: request.src(),
                    source,
                },
            );
        }
    };
    if let Err(source) = device.memory.write_checked(request.dst(), 0, &bytes) {
        return publish_fault(
            device,
            FaultRetirement {
                queue_id,
                command,
                buffer: request.dst(),
                source,
            },
        );
    }
    let completion = CompletionRecord::new(
        command.queue_id(),
        command.command_id(),
        CompletionStatus::Success,
    );
    if device
        .completion_mut(command.completion_ring_id())?
        .is_full()
    {
        device.runnable.push_front(queue_id);
        return Ok(BackendStepStatus::Stalled);
    }
    device
        .completion_mut(command.completion_ring_id())?
        .push(completion)?;
    retire_command(device, queue_id)?;
    Ok(BackendStepStatus::Progressed)
}

fn publish_fault(
    device: &mut NativeDevice,
    fault: FaultRetirement,
) -> ModelResult<BackendStepStatus> {
    let event = EventRecord::new(
        fault.command.queue_id(),
        fault.command.command_id(),
        event_error_kind(fault.source)?,
        fault.buffer,
    );
    device.events.push_back(event);
    retire_command(device, fault.queue_id)?;
    Err(fault.source)
}

fn retire_command(device: &mut NativeDevice, queue_id: QueueId) -> ModelResult<()> {
    let queue = device.queue_mut(queue_id)?;
    let _retired = queue.commands.pop();
    if !queue.commands.is_empty() {
        device.runnable.push_back(queue_id);
    }
    Ok(())
}

const fn event_error_kind(error: ModelError) -> ModelResult<EventErrorKind> {
    match error {
        ModelError::UnknownBuffer(_) => Ok(EventErrorKind::UnknownBuffer),
        ModelError::RangeOverflow => Ok(EventErrorKind::RangeOverflow),
        ModelError::OutOfBounds { len, end, .. } => Ok(EventErrorKind::OutOfBounds { len, end }),
        _ => Err(error),
    }
}
