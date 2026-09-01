use super::context::{CommandRoute, ExecutionOutcome, success};
use super::memory::QueueMemoryResolver;
use crate::dma::PdmaMemoryMappings;
use pdma_model_contract::semantic::{DmaRangeStart, TraceOperation, VectorAddRequest};

pub(super) fn execute(
    dma: &PdmaMemoryMappings,
    route: CommandRoute,
    request: VectorAddRequest,
) -> ExecutionOutcome {
    let memory = QueueMemoryResolver::new(route.queue, route.command);
    let source_a = DmaRangeStart::new(request.src_a(), request.src_a_offset());
    let source_b = DmaRangeStart::new(request.src_b(), request.src_b_offset());
    let destination = DmaRangeStart::new(request.dst(), request.dst_offset());
    let src_a = match memory.read(dma, source_a, request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    let src_b = match memory.read(dma, source_b, request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    let sum = src_a
        .iter()
        .zip(src_b.iter())
        .map(|(left, right)| left.wrapping_add(*right))
        .collect::<Vec<_>>();
    if let Err(fault) = memory.validate(dma, destination, request.len()) {
        return ExecutionOutcome::Fault(fault);
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    if let Err(fault) = memory.write(dma, destination, &sum) {
        return ExecutionOutcome::Fault(fault);
    }
    success(route, TraceOperation::VectorAdd)
}
