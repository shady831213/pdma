use super::memory::QueueMemoryResolver;
use super::{CommandRoute, ExecutionFault, ExecutionOutcome, success};
use crate::memory::PdmaMemoryMappings;
use pdma_model_contract::semantic::{
    CopyRequest, DmaRangeStart, StridedCopyRequest, TraceOperation,
};

#[derive(Debug, Clone, Copy)]
struct NativeCopyChunk {
    src: DmaRangeStart,
    dst: DmaRangeStart,
    len: usize,
}

pub(super) fn execute(
    memory: &PdmaMemoryMappings,
    route: CommandRoute,
    request: CopyRequest,
) -> ExecutionOutcome {
    let resolver = QueueMemoryResolver::new(route.queue, route.command);
    let bytes = match resolver.read(memory, request.src_start(), request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    if let Err(fault) = resolver.validate(memory, request.dst_start(), request.len()) {
        return ExecutionOutcome::Fault(fault);
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    if let Err(fault) = resolver.write(memory, request.dst_start(), &bytes) {
        return ExecutionOutcome::Fault(fault);
    }
    success(route, TraceOperation::Copy)
}

pub(super) fn execute_strided(
    memory: &PdmaMemoryMappings,
    route: CommandRoute,
    request: StridedCopyRequest,
) -> ExecutionOutcome {
    let resolver = QueueMemoryResolver::new(route.queue, route.command);
    let chunks = match strided_chunks(memory, resolver, request) {
        Ok(chunks) => chunks,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    for chunk in &chunks {
        if let Err(fault) = resolver.validate(memory, chunk.src, chunk.len) {
            return ExecutionOutcome::Fault(fault);
        }
        if let Err(fault) = resolver.validate(memory, chunk.dst, chunk.len) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    for chunk in chunks {
        let bytes = match resolver.read(memory, chunk.src, chunk.len) {
            Ok(bytes) => bytes,
            Err(fault) => return ExecutionOutcome::Fault(fault),
        };
        if let Err(fault) = resolver.write(memory, chunk.dst, &bytes) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    success(route, TraceOperation::StridedCopy)
}

fn strided_chunks(
    memory: &PdmaMemoryMappings,
    resolver: QueueMemoryResolver,
    request: StridedCopyRequest,
) -> Result<Vec<NativeCopyChunk>, ExecutionFault> {
    let (Some(source), Some(destination)) = (request.src_start(), request.dst_start()) else {
        return Ok(Vec::new());
    };
    let mut chunks = Vec::new();
    for row in 0..request.rows() {
        let src = resolver.strided_start(memory, source, row, request.src_stride())?;
        let dst = resolver.strided_start(memory, destination, row, request.dst_stride())?;
        chunks.push(NativeCopyChunk {
            src,
            dst,
            len: request.row_bytes(),
        });
    }
    Ok(chunks)
}
