use super::context::{CommandRoute, ExecutionOutcome, success};
use super::memory::QueueMemoryResolver;
use crate::dma::PdmaMemoryMappings;
use pdma_model_contract::semantic::{BroadcastCopyRequest, DmaRangeStart, TraceOperation};

pub(super) fn execute(
    dma: &PdmaMemoryMappings,
    route: CommandRoute,
    request: &BroadcastCopyRequest,
) -> ExecutionOutcome {
    let memory = QueueMemoryResolver::new(route.queue, route.command);
    let source = DmaRangeStart::new(request.src(), request.src_offset());
    for destination in request.destinations() {
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        if let Err(fault) = memory.validate(dma, source, request.len()) {
            return ExecutionOutcome::Fault(fault);
        }
        if let Err(fault) = memory.validate(dma, destination, request.len()) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    for destination in request.destinations() {
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        let bytes = match memory.read(dma, source, request.len()) {
            Ok(bytes) => bytes,
            Err(fault) => return ExecutionOutcome::Fault(fault),
        };
        if let Err(fault) = memory.write(dma, destination, &bytes) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    success(route, TraceOperation::BroadcastCopy)
}
