use super::NativeDevice;
use crate::runner::backend::BackendStepStatus;
use pdma_model_contract::completion::{CompletionRecord, CompletionStatus};
use pdma_model_contract::desc::{CopyCommand, ExternalCommand, StridedCopyCommand};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::{EventErrorKind, EventRecord};
use pdma_model_contract::ids::{BufferId, CommandId, CompletionRingId, QueueId};

#[derive(Debug, Clone, Copy)]
struct FaultRetirement {
    queue_id: QueueId,
    command_id: CommandId,
    buffer: BufferId,
    source: ModelError,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SuccessRetirement {
    queue: QueueId,
    command: CommandId,
    completion_ring: CompletionRingId,
}

#[derive(Debug, Clone, Copy)]
struct NativeCopyChunk {
    src: BufferId,
    dst: BufferId,
    src_offset: usize,
    dst_offset: usize,
    len: usize,
}

pub(super) type PendingCompletion = SuccessRetirement;

pub(super) fn step(device: &mut NativeDevice) -> ModelResult<BackendStepStatus> {
    if device.pending_completion.is_some() {
        return retry_pending_completion(device);
    }
    let Some(queue_id) = device.runnable.pop_front() else {
        return Ok(BackendStepStatus::Idle);
    };
    let Some(command) = device.queue(queue_id)?.commands.front().cloned() else {
        return Ok(BackendStepStatus::Progressed);
    };
    match command {
        ExternalCommand::Copy(copy) => retire_copy(device, queue_id, copy),
        ExternalCommand::StridedCopy(strided) => retire_strided_copy(device, queue_id, strided),
        ExternalCommand::BroadcastCopy(_) | ExternalCommand::VectorAdd(_) => {
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
                    command_id: command.command_id(),
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
                command_id: command.command_id(),
                buffer: request.dst(),
                source,
            },
        );
    }
    publish_completion(
        device,
        SuccessRetirement {
            queue: queue_id,
            command: command.command_id(),
            completion_ring: command.completion_ring_id(),
        },
    )
}

fn retire_strided_copy(
    device: &mut NativeDevice,
    queue_id: QueueId,
    command: StridedCopyCommand,
) -> ModelResult<BackendStepStatus> {
    let chunks = match strided_chunks(queue_id, command) {
        Ok(chunks) => chunks,
        Err(fault) => return publish_fault(device, fault),
    };
    for chunk in chunks {
        let bytes = match device
            .memory
            .read_checked(chunk.src, chunk.src_offset, chunk.len)
        {
            Ok(bytes) => bytes,
            Err(source) => {
                return publish_fault(
                    device,
                    FaultRetirement {
                        queue_id,
                        command_id: command.command_id(),
                        buffer: chunk.src,
                        source,
                    },
                );
            }
        };
        if let Err(source) = device
            .memory
            .write_checked(chunk.dst, chunk.dst_offset, &bytes)
        {
            return publish_fault(
                device,
                FaultRetirement {
                    queue_id,
                    command_id: command.command_id(),
                    buffer: chunk.dst,
                    source,
                },
            );
        }
    }
    publish_completion(
        device,
        SuccessRetirement {
            queue: queue_id,
            command: command.command_id(),
            completion_ring: command.completion_ring_id(),
        },
    )
}

fn strided_chunks(
    queue_id: QueueId,
    command: StridedCopyCommand,
) -> Result<Vec<NativeCopyChunk>, FaultRetirement> {
    let request = command.request();
    let mut chunks = Vec::new();
    for row in 0..request.rows() {
        let src_offset = row
            .checked_mul(request.src_stride())
            .ok_or_else(|| FaultRetirement {
                queue_id,
                command_id: command.command_id(),
                buffer: request.src(),
                source: ModelError::RangeOverflow,
            })?;
        let dst_offset = row
            .checked_mul(request.dst_stride())
            .ok_or_else(|| FaultRetirement {
                queue_id,
                command_id: command.command_id(),
                buffer: request.dst(),
                source: ModelError::RangeOverflow,
            })?;
        chunks.push(NativeCopyChunk {
            src: request.src(),
            dst: request.dst(),
            src_offset,
            dst_offset,
            len: request.row_bytes(),
        });
    }
    Ok(chunks)
}

fn publish_completion(
    device: &mut NativeDevice,
    success: SuccessRetirement,
) -> ModelResult<BackendStepStatus> {
    let completion =
        CompletionRecord::new(success.queue, success.command, CompletionStatus::Success);
    if device.completion_mut(success.completion_ring)?.is_full() {
        retire_command_without_requeue(device, success.queue)?;
        device.pending_completion = Some(success);
        return Ok(BackendStepStatus::Stalled);
    }
    device
        .completion_mut(success.completion_ring)?
        .push(completion)?;
    retire_command(device, success.queue)?;
    Ok(BackendStepStatus::Progressed)
}

fn retry_pending_completion(device: &mut NativeDevice) -> ModelResult<BackendStepStatus> {
    let Some(pending) = device.pending_completion else {
        return Ok(BackendStepStatus::Idle);
    };
    let completion =
        CompletionRecord::new(pending.queue, pending.command, CompletionStatus::Success);
    if device.completion_mut(pending.completion_ring)?.is_full() {
        return Ok(BackendStepStatus::Stalled);
    }
    device
        .completion_mut(pending.completion_ring)?
        .push(completion)?;
    device.pending_completion = None;
    if !device.queue(pending.queue)?.commands.is_empty() {
        device.runnable.push_back(pending.queue);
    }
    Ok(BackendStepStatus::Progressed)
}

fn publish_fault(
    device: &mut NativeDevice,
    fault: FaultRetirement,
) -> ModelResult<BackendStepStatus> {
    let event = EventRecord::new(
        fault.queue_id,
        fault.command_id,
        event_error_kind(fault.source)?,
        fault.buffer,
    );
    device.events.push_back(event);
    retire_command(device, fault.queue_id)?;
    Err(fault.source)
}

fn retire_command(device: &mut NativeDevice, queue_id: QueueId) -> ModelResult<()> {
    retire_command_without_requeue(device, queue_id)?;
    if !device.queue(queue_id)?.commands.is_empty() {
        device.runnable.push_back(queue_id);
    }
    Ok(())
}

fn retire_command_without_requeue(device: &mut NativeDevice, queue_id: QueueId) -> ModelResult<()> {
    let queue = device.queue_mut(queue_id)?;
    let _retired = queue.commands.pop();
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
