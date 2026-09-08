use super::memory::QueueMemoryResolver;
use super::{CommandRoute, ExecutionOutcome, success};
use crate::memory::PdmaMemoryMappings;
use crate::operation::VectorOperands;
use pdma_model_contract::abi::{AbiEventAccess, AbiEventStage, AbiFaultSite};
use pdma_model_contract::semantic::TraceOperation;

pub(super) fn execute(
    memory: &PdmaMemoryMappings,
    route: CommandRoute,
    request: VectorOperands,
) -> ExecutionOutcome {
    let resolver = QueueMemoryResolver::new(route.queue, route.command);
    let source_a = request.source_a;
    let source_b = request.source_b;
    let destination = request.destination;
    let first_source_site = AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, 0);
    let second_source_site = AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, 1);
    let destination_site = AbiFaultSite::new(AbiEventStage::Destination, AbiEventAccess::Write, 0);
    let src_a = match resolver.read(
        memory,
        source_a,
        u64::from(request.length),
        first_source_site,
    ) {
        Ok(bytes) => bytes,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    let src_b = match resolver.read(
        memory,
        source_b,
        u64::from(request.length),
        second_source_site,
    ) {
        Ok(bytes) => bytes,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    let sum = src_a
        .iter()
        .zip(src_b.iter())
        .map(|(left, right)| left.wrapping_add(*right))
        .collect::<Vec<_>>();
    if let Err(fault) = resolver.validate(
        memory,
        destination,
        u64::from(request.length),
        destination_site,
    ) {
        return ExecutionOutcome::Fault(fault);
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    if let Err(fault) = resolver.write(memory, destination, &sum, destination_site) {
        return ExecutionOutcome::Fault(fault);
    }
    success(route, TraceOperation::VectorAdd)
}
