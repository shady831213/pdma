use super::NativeExecution;
use super::context::CommandRoute;
use super::io::QueueMemoryResolver;
use super::retirement::validate_success_completion_route;
use super::retirement::{FaultRetirement, SuccessRetirement, publish_completion, publish_fault};
use crate::ModelResult;
use crate::engine::step::ModelStepStatus;
use pdma_model_contract::semantic::{
    CopyRequest, DmaRangeStart, StridedCopyRequest, TraceOperation,
};

#[derive(Debug, Clone, Copy)]
struct NativeCopyChunk {
    src: DmaRangeStart,
    dst: DmaRangeStart,
    len: usize,
}

pub(super) fn retire_copy(
    execution: NativeExecution<'_>,
    route: CommandRoute,
    request: CopyRequest,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        queue_control,
    } = execution;
    let memory = super::DeviceExecution::memory_resolver(route.queue, route.command);
    let bytes = match memory.read(dma_space, request.src_start(), request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, queue_control, fault),
    };
    if let Err(fault) = memory.validate(dma_space, request.dst_start(), request.len()) {
        return publish_fault(device, dma_space, queue_control, fault);
    }
    validate_success_completion_route(
        device,
        dma_space,
        queue_control,
        route.queue,
        route.completion_ring,
    )?;
    if let Err(fault) = memory.write(dma_space, request.dst_start(), &bytes) {
        return publish_fault(device, dma_space, queue_control, fault);
    }
    publish_completion(
        device,
        dma_space,
        queue_control,
        SuccessRetirement {
            queue: route.queue,
            command: route.command,
            completion_ring: route.completion_ring,
            operation: TraceOperation::Copy,
        },
    )
}

pub(super) fn retire_strided_copy(
    execution: NativeExecution<'_>,
    route: CommandRoute,
    request: StridedCopyRequest,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        queue_control,
    } = execution;
    let memory = super::DeviceExecution::memory_resolver(route.queue, route.command);
    let chunks = match strided_chunks(dma_space, memory, request) {
        Ok(chunks) => chunks,
        Err(fault) => return publish_fault(device, dma_space, queue_control, fault),
    };
    for chunk in &chunks {
        if let Err(fault) = memory.validate(dma_space, chunk.src, chunk.len) {
            return publish_fault(device, dma_space, queue_control, fault);
        }
        if let Err(fault) = memory.validate(dma_space, chunk.dst, chunk.len) {
            return publish_fault(device, dma_space, queue_control, fault);
        }
    }
    validate_success_completion_route(
        device,
        dma_space,
        queue_control,
        route.queue,
        route.completion_ring,
    )?;
    for chunk in chunks {
        let bytes = match memory.read(dma_space, chunk.src, chunk.len) {
            Ok(bytes) => bytes,
            Err(fault) => return publish_fault(device, dma_space, queue_control, fault),
        };
        if let Err(fault) = memory.write(dma_space, chunk.dst, &bytes) {
            return publish_fault(device, dma_space, queue_control, fault);
        }
    }
    publish_completion(
        device,
        dma_space,
        queue_control,
        SuccessRetirement {
            queue: route.queue,
            command: route.command,
            completion_ring: route.completion_ring,
            operation: TraceOperation::StridedCopy,
        },
    )
}

fn strided_chunks(
    dma_space: &crate::abi::dma::PdmaMemoryMappings,
    memory: QueueMemoryResolver,
    request: StridedCopyRequest,
) -> Result<Vec<NativeCopyChunk>, FaultRetirement> {
    let (Some(source), Some(destination)) = (request.src_start(), request.dst_start()) else {
        return Ok(Vec::new());
    };
    let mut chunks = Vec::new();
    for row in 0..request.rows() {
        let src = memory.strided_start(dma_space, source, row, request.src_stride())?;
        let dst = memory.strided_start(dma_space, destination, row, request.dst_stride())?;
        chunks.push(NativeCopyChunk {
            src,
            dst,
            len: request.row_bytes(),
        });
    }
    Ok(chunks)
}
