use super::NativeExecution;
use super::retirement::validate_success_completion_route;
use super::retirement::{FaultRetirement, SuccessRetirement, publish_completion, publish_fault};
use crate::runner::backend::ModelStepStatus;
use pdma_model_contract::desc::VectorAddCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::ids::QueueId;
use pdma_model_contract::trace::TraceOperation;

pub(super) fn retire_vector_add(
    execution: NativeExecution<'_>,
    queue_id: QueueId,
    command: VectorAddCommand,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution { device, dma_space } = execution;
    let request = command.request();
    let src_a = match dma_space.read_object(request.src_a(), request.src_a_offset(), request.len())
    {
        Ok(bytes) => bytes,
        Err(source) => {
            return publish_fault(
                device,
                FaultRetirement {
                    queue_id,
                    command_id: command.command_id(),
                    buffer: request.src_a(),
                    source,
                },
            );
        }
    };
    let src_b = match dma_space.read_object(request.src_b(), request.src_b_offset(), request.len())
    {
        Ok(bytes) => bytes,
        Err(source) => {
            return publish_fault(
                device,
                FaultRetirement {
                    queue_id,
                    command_id: command.command_id(),
                    buffer: request.src_b(),
                    source,
                },
            );
        }
    };
    let sum = src_a
        .iter()
        .zip(src_b.iter())
        .map(|(left, right)| left.wrapping_add(*right))
        .collect::<Vec<_>>();
    validate_success_completion_route(device, queue_id, command.completion_ring_id())?;
    if let Err(source) = dma_space.write_object(request.dst(), request.dst_offset(), &sum) {
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
            operation: TraceOperation::VectorAdd,
        },
    )
}
