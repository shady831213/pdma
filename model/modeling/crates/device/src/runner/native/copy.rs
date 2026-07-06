use super::QueuePortDeviceModel;
use super::retirement::validate_success_completion_route;
use super::retirement::{FaultRetirement, SuccessRetirement, publish_completion, publish_fault};
use crate::runner::backend::ModelStepStatus;
use pdma_model_contract::desc::{CopyCommand, StridedCopyCommand};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::ids::{BufferId, QueueId};
use pdma_model_contract::trace::TraceOperation;

#[derive(Debug, Clone, Copy)]
struct NativeCopyChunk {
    src: BufferId,
    dst: BufferId,
    src_offset: usize,
    dst_offset: usize,
    len: usize,
}

pub(super) fn retire_copy(
    device: &mut QueuePortDeviceModel,
    queue_id: QueueId,
    command: CopyCommand,
) -> ModelResult<ModelStepStatus> {
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
    validate_success_completion_route(device, queue_id, command.completion_ring_id())?;
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
            operation: TraceOperation::Copy,
        },
    )
}

pub(super) fn retire_strided_copy(
    device: &mut QueuePortDeviceModel,
    queue_id: QueueId,
    command: StridedCopyCommand,
) -> ModelResult<ModelStepStatus> {
    let chunks = match strided_chunks(queue_id, command) {
        Ok(chunks) => chunks,
        Err(fault) => return publish_fault(device, fault),
    };
    validate_success_completion_route(device, queue_id, command.completion_ring_id())?;
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
            operation: TraceOperation::StridedCopy,
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
