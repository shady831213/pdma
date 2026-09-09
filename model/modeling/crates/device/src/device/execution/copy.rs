#[cfg(test)]
mod tests;

use super::memory::QueueMemoryResolver;
use super::{CommandRoute, ExecutionOutcome, success};
use crate::device::flow::ExecutionFault;
use crate::operation::{CopyOperands, StridedOperands};
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_contract::abi::{AbiEventAccess, AbiEventStage, AbiFaultSite};
use pdma_model_contract::semantic::TraceOperation;

pub(super) fn execute(
    memory: &SharedMemoryAccess,
    route: CommandRoute,
    request: CopyOperands,
) -> ExecutionOutcome {
    let resolver = QueueMemoryResolver::new(route.queue, route.command);
    let source = source_site(0);
    let destination = destination_site(0);
    let bytes = match resolver.read(memory, request.source, u64::from(request.length), source) {
        Ok(bytes) => bytes,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    if let Err(fault) = resolver.validate(
        memory,
        request.destination,
        u64::from(request.length),
        destination,
    ) {
        return ExecutionOutcome::Fault(fault);
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    if let Err(fault) = resolver.write(memory, request.destination, &bytes, destination) {
        return ExecutionOutcome::Fault(fault);
    }
    success(route, TraceOperation::Copy)
}

pub(super) fn execute_strided(
    memory: &SharedMemoryAccess,
    route: CommandRoute,
    request: StridedOperands,
) -> ExecutionOutcome {
    let resolver = QueueMemoryResolver::new(route.queue, route.command);
    let chunks = (0..request.rows)
        .map(|row| resolve_row(memory, resolver, request, row))
        .collect::<Result<Vec<_>, _>>();
    let chunks = match chunks {
        Ok(chunks) => chunks,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    for (row, chunk) in (0..request.rows).zip(chunks) {
        let bytes = match resolver.read(
            memory,
            chunk.source,
            u64::from(chunk.length),
            source_site(row),
        ) {
            Ok(bytes) => bytes,
            Err(fault) => return ExecutionOutcome::Fault(fault),
        };
        if let Err(fault) = resolver.write(memory, chunk.destination, &bytes, destination_site(row))
        {
            return ExecutionOutcome::Fault(fault);
        }
    }
    success(route, TraceOperation::StridedCopy)
}

fn resolve_row(
    memory: &SharedMemoryAccess,
    resolver: QueueMemoryResolver,
    request: StridedOperands,
    row: u32,
) -> Result<CopyOperands, ExecutionFault> {
    let source =
        resolver.strided_address(request.source, row, request.source_stride, source_site(row))?;
    resolver.validate(
        memory,
        source,
        u64::from(request.row_bytes),
        source_site(row),
    )?;
    let destination = resolver.strided_address(
        request.destination,
        row,
        request.destination_stride,
        destination_site(row),
    )?;
    resolver.validate(
        memory,
        destination,
        u64::from(request.row_bytes),
        destination_site(row),
    )?;
    Ok(CopyOperands {
        source,
        destination,
        length: request.row_bytes,
    })
}

fn source_site(row: u32) -> AbiFaultSite {
    AbiFaultSite::new(AbiEventStage::Source, AbiEventAccess::Read, u64::from(row))
}

fn destination_site(row: u32) -> AbiFaultSite {
    AbiFaultSite::new(
        AbiEventStage::Destination,
        AbiEventAccess::Write,
        u64::from(row),
    )
}
