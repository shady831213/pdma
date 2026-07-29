use super::NativeExecution;
use super::retirement::validate_success_completion_route;
use super::retirement::{SuccessRetirement, publish_completion, publish_fault};
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
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let request = command.request();
    let memory = device.memory_resolver(queue_id, command.command_id())?;
    let left_source =
        pdma_model_contract::DmaRangeStart::new(request.src_a(), request.src_a_offset());
    let src_a = match memory.read(dma_space, left_source, request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
    };
    let right_source =
        pdma_model_contract::DmaRangeStart::new(request.src_b(), request.src_b_offset());
    let src_b = match memory.read(dma_space, right_source, request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
    };
    let sum = src_a
        .iter()
        .zip(src_b.iter())
        .map(|(left, right)| left.wrapping_add(*right))
        .collect::<Vec<_>>();
    let destination = pdma_model_contract::DmaRangeStart::new(request.dst(), request.dst_offset());
    if let Err(fault) = memory.validate(dma_space, destination, request.len()) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    validate_success_completion_route(
        device,
        dma_space,
        bus_control,
        queue_id,
        command.completion_ring_id(),
    )?;
    if let Err(fault) = memory.write(dma_space, destination, &sum) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    publish_completion(
        device,
        dma_space,
        bus_control,
        SuccessRetirement {
            queue: queue_id,
            command: command.command_id(),
            completion_ring: command.completion_ring_id(),
            operation: TraceOperation::VectorAdd,
        },
    )
}
