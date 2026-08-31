use super::NativeExecution;
use super::context::CommandRoute;
use super::retirement::validate_success_completion_route;
use super::retirement::{SuccessRetirement, publish_completion, publish_fault};
use crate::ModelResult;
use crate::engine::step::ModelStepStatus;
use pdma_model_contract::semantic::{BroadcastCopyRequest, DmaRangeStart, TraceOperation};

pub(super) fn retire_broadcast_copy(
    execution: NativeExecution<'_>,
    route: CommandRoute,
    request: &BroadcastCopyRequest,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        queue_control,
    } = execution;
    let memory = super::DeviceExecution::memory_resolver(route.queue, route.command);
    let source = DmaRangeStart::new(request.src(), request.src_offset());
    for destination in request.destinations() {
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        if let Err(fault) = memory.validate(dma_space, source, request.len()) {
            return publish_fault(device, dma_space, queue_control, fault);
        }
        if let Err(fault) = memory.validate(dma_space, destination, request.len()) {
            return publish_fault(device, dma_space, queue_control, fault);
        }
    }
    validate_success_completion_route(
        device,
        dma_space,
        queue_control,
        route.queue,
        route.completion_ring,
    )?;
    for destination in request.destinations() {
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        let bytes = match memory.read(dma_space, source, request.len()) {
            Ok(bytes) => bytes,
            Err(fault) => return publish_fault(device, dma_space, queue_control, fault),
        };
        if let Err(fault) = memory.write(dma_space, destination, &bytes) {
            return publish_fault(device, dma_space, queue_control, fault);
        }
    }
    publish_completion(
        device,
        dma_space,
        queue_control,
        SuccessRetirement {
            queue: route.queue,
            command: route.command,
            completion_ring: route.completion_ring,
            operation: TraceOperation::BroadcastCopy,
        },
    )
}
