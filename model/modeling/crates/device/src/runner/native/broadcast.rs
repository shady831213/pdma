use super::NativeExecution;
use super::retirement::validate_success_completion_route;
use super::retirement::{SuccessRetirement, publish_completion, publish_fault};
use crate::ModelResult;
use crate::runner::backend::ModelStepStatus;
use pdma_model_contract::semantic::DmaRangeStart;
use pdma_model_contract::semantic::desc::BroadcastCopyCommand;
use pdma_model_contract::semantic::trace::TraceOperation;

pub(super) fn retire_broadcast_copy(
    execution: NativeExecution<'_>,
    queue_id: pdma_model_contract::semantic::QueueId,
    command: &BroadcastCopyCommand,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let request = command.request();
    let memory = super::QueuePortDeviceModel::memory_resolver(queue_id, command.command_id());
    let source = DmaRangeStart::new(request.src(), request.src_offset());
    for destination in request.destinations() {
        if let Err(fault) = memory.validate(dma_space, source, request.len()) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        if let Err(fault) = memory.validate(dma_space, destination, request.len()) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
    }
    validate_success_completion_route(
        device,
        dma_space,
        bus_control,
        queue_id,
        command.completion_ring_id(),
    )?;
    for destination in request.destinations() {
        let bytes = match memory.read(dma_space, source, request.len()) {
            Ok(bytes) => bytes,
            Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
        };
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        if let Err(fault) = memory.write(dma_space, destination, &bytes) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
    }
    publish_completion(
        device,
        dma_space,
        bus_control,
        SuccessRetirement {
            queue: queue_id,
            command: command.command_id(),
            completion_ring: command.completion_ring_id(),
            operation: TraceOperation::BroadcastCopy,
        },
    )
}
