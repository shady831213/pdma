use super::context::{CommandRoute, ExecutionFault, ExecutionOutcome, success};
use super::memory::QueueMemoryResolver;
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
    dma: &PdmaMemoryMappings,
    route: CommandRoute,
    request: CopyRequest,
) -> ExecutionOutcome {
    let memory = QueueMemoryResolver::new(route.queue, route.command);
    let bytes = match memory.read(dma, request.src_start(), request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    if let Err(fault) = memory.validate(dma, request.dst_start(), request.len()) {
        return ExecutionOutcome::Fault(fault);
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    if let Err(fault) = memory.write(dma, request.dst_start(), &bytes) {
        return ExecutionOutcome::Fault(fault);
    }
    success(route, TraceOperation::Copy)
}

pub(super) fn execute_strided(
    dma: &PdmaMemoryMappings,
    route: CommandRoute,
    request: StridedCopyRequest,
) -> ExecutionOutcome {
    let memory = QueueMemoryResolver::new(route.queue, route.command);
    let chunks = match strided_chunks(dma, memory, request) {
        Ok(chunks) => chunks,
        Err(fault) => return ExecutionOutcome::Fault(fault),
    };
    for chunk in &chunks {
        if let Err(fault) = memory.validate(dma, chunk.src, chunk.len) {
            return ExecutionOutcome::Fault(fault);
        }
        if let Err(fault) = memory.validate(dma, chunk.dst, chunk.len) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    if !route.completion_is_valid() {
        return ExecutionOutcome::InvalidCompletion(route);
    }
    for chunk in chunks {
        let bytes = match memory.read(dma, chunk.src, chunk.len) {
            Ok(bytes) => bytes,
            Err(fault) => return ExecutionOutcome::Fault(fault),
        };
        if let Err(fault) = memory.write(dma, chunk.dst, &bytes) {
            return ExecutionOutcome::Fault(fault);
        }
    }
    success(route, TraceOperation::StridedCopy)
}

fn strided_chunks(
    dma: &PdmaMemoryMappings,
    memory: QueueMemoryResolver,
    request: StridedCopyRequest,
) -> Result<Vec<NativeCopyChunk>, ExecutionFault> {
    let (Some(source), Some(destination)) = (request.src_start(), request.dst_start()) else {
        return Ok(Vec::new());
    };
    let mut chunks = Vec::new();
    for row in 0..request.rows() {
        let src = memory.strided_start(dma, source, row, request.src_stride())?;
        let dst = memory.strided_start(dma, destination, row, request.dst_stride())?;
        chunks.push(NativeCopyChunk {
            src,
            dst,
            len: request.row_bytes(),
        });
    }
    Ok(chunks)
}
