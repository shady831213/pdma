use super::memory::QueueMemoryResolver;
use super::{CommandRoute, ExecutionOutcome, success};
use crate::operation::BroadcastOperands;
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_contract::abi::{AbiEventAccess, AbiEventStage, AbiFaultSite, DmaAddress};
use pdma_model_contract::semantic::TraceOperation;

pub(super) fn execute(
    memory: &SharedMemoryAccess,
    route: CommandRoute,
    request: &BroadcastOperands,
) -> ExecutionOutcome {
    let resolver = QueueMemoryResolver::new(route.queue, route.command);
    let source_site = AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, 0);
    if let Err(fault) = resolver.validate(
        memory,
        request.source,
        u64::from(request.length),
        source_site,
    ) {
        return ExecutionOutcome::Fault(fault);
    }
    let bytes =
        match resolver.indirect_list(memory, request.destination_list, request.destination_count) {
            Ok(bytes) => bytes,
            Err(fault) => return ExecutionOutcome::Fault(fault),
        };
    let destinations = bytes
        .chunks_exact(8)
        .map(|chunk| {
            let mut entry = [0; 8];
            entry.copy_from_slice(chunk);
            DmaAddress::new(u64::from_le_bytes(entry))
        })
        .collect::<Vec<_>>();
    for (index, destination) in (0..request.destination_count).zip(&destinations) {
        if let Err(fault) = resolver.validate(
            memory,
            *destination,
            u64::from(request.length),
            destination_site(index),
        ) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    for (index, destination) in (0..request.destination_count).zip(destinations) {
        let bytes = match resolver.read(
            memory,
            request.source,
            u64::from(request.length),
            source_site,
        ) {
            Ok(bytes) => bytes,
            Err(fault) => return ExecutionOutcome::Fault(fault),
        };
        if let Err(fault) = resolver.write(memory, destination, &bytes, destination_site(index)) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    success(route, TraceOperation::BroadcastCopy)
}

fn destination_site(index: u32) -> AbiFaultSite {
    AbiFaultSite::new(
        AbiEventStage::Destination,
        AbiEventAccess::Write,
        u64::from(index),
    )
}
