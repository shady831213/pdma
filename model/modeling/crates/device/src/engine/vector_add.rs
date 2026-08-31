use super::NativeExecution;
use super::context::CommandRoute;
use super::retirement::validate_success_completion_route;
use super::retirement::{SuccessRetirement, publish_completion, publish_fault};
use crate::ModelResult;
use crate::engine::step::ModelStepStatus;
use pdma_model_contract::semantic::{DmaRangeStart, TraceOperation, VectorAddRequest};

pub(super) fn retire_vector_add(
    execution: NativeExecution<'_>,
    route: CommandRoute,
    request: VectorAddRequest,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        queue_control,
    } = execution;
    let memory = super::DeviceExecution::memory_resolver(route.queue, route.command);
    let source_a = DmaRangeStart::new(request.src_a(), request.src_a_offset());
    let source_b = DmaRangeStart::new(request.src_b(), request.src_b_offset());
    let destination = DmaRangeStart::new(request.dst(), request.dst_offset());
    let src_a = match memory.read(dma_space, source_a, request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, queue_control, fault),
    };
    let src_b = match memory.read(dma_space, source_b, request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, queue_control, fault),
    };
    let sum = src_a
        .iter()
        .zip(src_b.iter())
        .map(|(left, right)| left.wrapping_add(*right))
        .collect::<Vec<_>>();
    if let Err(fault) = memory.validate(dma_space, destination, request.len()) {
        return publish_fault(device, dma_space, queue_control, fault);
    }
    validate_success_completion_route(
        device,
        dma_space,
        queue_control,
        route.queue,
        route.completion_ring,
    )?;
    if let Err(fault) = memory.write(dma_space, destination, &sum) {
        return publish_fault(device, dma_space, queue_control, fault);
    }
    publish_completion(
        device,
        dma_space,
        queue_control,
        SuccessRetirement {
            queue: route.queue,
            command: route.command,
            completion_ring: route.completion_ring,
            operation: TraceOperation::VectorAdd,
        },
    )
}
