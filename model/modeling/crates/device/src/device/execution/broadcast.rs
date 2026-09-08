use super::memory::QueueMemoryResolver;
use super::{CommandRoute, ExecutionOutcome, success};
use crate::memory::PdmaMemoryMappings;
use pdma_model_contract::semantic::{BroadcastCopyRequest, DmaRangeStart, TraceOperation};

pub(super) fn execute(
    memory: &PdmaMemoryMappings,
    route: CommandRoute,
    request: &BroadcastCopyRequest,
) -> ExecutionOutcome {
    let resolver = QueueMemoryResolver::new(route.queue, route.command);
    let source = DmaRangeStart::new(request.src(), request.src_offset());
    for destination in request.destinations() {
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        if let Err(fault) = resolver.validate(memory, source, request.len()) {
            return ExecutionOutcome::Fault(fault);
        }
        if let Err(fault) = resolver.validate(memory, destination, request.len()) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    for destination in request.destinations() {
        let destination = DmaRangeStart::new(destination.dst(), destination.dst_offset());
        let bytes = match resolver.read(memory, source, request.len()) {
            Ok(bytes) => bytes,
            Err(fault) => return ExecutionOutcome::Fault(fault),
        };
        if let Err(fault) = resolver.write(memory, destination, &bytes) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    success(route, TraceOperation::BroadcastCopy)
}
