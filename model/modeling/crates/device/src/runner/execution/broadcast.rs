use super::NativeExecution;
use super::command::{BroadcastCopyOperation, ExecutionRoute};
use super::retirement::validate_success_completion_route;
use super::retirement::{SuccessRetirement, publish_completion, publish_fault};
use crate::ModelResult;
use crate::runner::types::ModelStepStatus;
use pdma_model_contract::semantic::trace::TraceOperation;

pub(super) fn retire_broadcast_copy(
    execution: NativeExecution<'_>,
    route: ExecutionRoute,
    operation: BroadcastCopyOperation,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let memory = super::DeviceExecution::memory_resolver(route.queue, route.command);
    for destination in &operation.destinations {
        if let Err(fault) = memory.validate(dma_space, operation.source, operation.length) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
        if let Err(fault) = memory.validate(dma_space, *destination, operation.length) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
    }
    validate_success_completion_route(
        device,
        dma_space,
        bus_control,
        route.queue,
        route.completion_ring,
    )?;
    for destination in operation.destinations {
        let bytes = match memory.read(dma_space, operation.source, operation.length) {
            Ok(bytes) => bytes,
            Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
        };
        if let Err(fault) = memory.write(dma_space, destination, &bytes) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
    }
    publish_completion(
        device,
        dma_space,
        bus_control,
        SuccessRetirement {
            queue: route.queue,
            command: route.command,
            completion_ring: route.completion_ring,
            operation: TraceOperation::BroadcastCopy,
        },
    )
}
