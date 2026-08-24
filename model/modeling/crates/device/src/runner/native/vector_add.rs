use super::NativeExecution;
use super::execution::{ExecutionRoute, VectorAddOperation};
use super::retirement::validate_success_completion_route;
use super::retirement::{SuccessRetirement, publish_completion, publish_fault};
use crate::ModelResult;
use crate::runner::backend::ModelStepStatus;
use pdma_model_contract::semantic::trace::TraceOperation;

pub(super) fn retire_vector_add(
    execution: NativeExecution<'_>,
    route: ExecutionRoute,
    operation: VectorAddOperation,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let memory = super::QueuePortDeviceModel::memory_resolver(route.queue, route.command);
    let src_a = match memory.read(dma_space, operation.source_a, operation.length) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
    };
    let src_b = match memory.read(dma_space, operation.source_b, operation.length) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
    };
    let sum = src_a
        .iter()
        .zip(src_b.iter())
        .map(|(left, right)| left.wrapping_add(*right))
        .collect::<Vec<_>>();
    if let Err(fault) = memory.validate(dma_space, operation.destination, operation.length) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    validate_success_completion_route(
        device,
        dma_space,
        bus_control,
        route.queue,
        route.completion_ring,
    )?;
    if let Err(fault) = memory.write(dma_space, operation.destination, &sum) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    publish_completion(
        device,
        dma_space,
        bus_control,
        SuccessRetirement {
            queue: route.queue,
            command: route.command,
            completion_ring: route.completion_ring,
            operation: TraceOperation::VectorAdd,
        },
    )
}
