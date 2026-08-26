use super::NativeExecution;
use super::command::{CopyOperation, ExecutionRoute, ResolvedStridedRows, StridedCopyOperation};
use super::io::QueueMemoryResolver;
use super::retirement::validate_success_completion_route;
use super::retirement::{FaultRetirement, SuccessRetirement, publish_completion, publish_fault};
use crate::ModelResult;
use crate::runner::types::ModelStepStatus;
use pdma_model_contract::semantic::dma::DmaRangeStart;
use pdma_model_contract::semantic::trace::TraceOperation;

#[derive(Debug, Clone, Copy)]
struct NativeCopyChunk {
    src: DmaRangeStart,
    dst: DmaRangeStart,
    len: usize,
}

pub(super) fn retire_copy(
    execution: NativeExecution<'_>,
    route: ExecutionRoute,
    operation: CopyOperation,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let memory = super::DeviceExecution::memory_resolver(route.queue, route.command);
    let bytes = match memory.read(dma_space, operation.source, operation.length) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
    };
    if let Err(fault) = memory.validate(dma_space, operation.destination, operation.length) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    validate_success_completion_route(
        device,
        dma_space,
        bus_control,
        route.queue,
        route.completion_ring,
    )?;
    if let Err(fault) = memory.write(dma_space, operation.destination, &bytes) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    publish_completion(
        device,
        dma_space,
        bus_control,
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
    route: ExecutionRoute,
    operation: StridedCopyOperation,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let memory = super::DeviceExecution::memory_resolver(route.queue, route.command);
    let chunks = match strided_chunks(dma_space, memory, operation) {
        Ok(chunks) => chunks,
        Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
    };
    for chunk in &chunks {
        if let Err(fault) = memory.validate(dma_space, chunk.src, chunk.len) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
        if let Err(fault) = memory.validate(dma_space, chunk.dst, chunk.len) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
    }
    validate_success_completion_route(
        device,
        dma_space,
        bus_control,
        route.queue,
        route.completion_ring,
    )?;
    for chunk in chunks {
        let bytes = match memory.read(dma_space, chunk.src, chunk.len) {
            Ok(bytes) => bytes,
            Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
        };
        if let Err(fault) = memory.write(dma_space, chunk.dst, &bytes) {
            return publish_fault(device, dma_space, bus_control, fault);
        }
    }
    publish_completion(
        device,
        dma_space,
        bus_control,
        SuccessRetirement {
            queue: route.queue,
            command: route.command,
            completion_ring: route.completion_ring,
            operation: TraceOperation::StridedCopy,
        },
    )
}

fn strided_chunks(
    dma_space: &crate::dma_space::PdmaMemoryMappings,
    memory: QueueMemoryResolver,
    operation: StridedCopyOperation,
) -> Result<Vec<NativeCopyChunk>, FaultRetirement> {
    let (source, destination, count) = match operation.rows {
        ResolvedStridedRows::Empty => return Ok(Vec::new()),
        ResolvedStridedRows::NonEmpty {
            source,
            destination,
            count,
        } => (source, destination, count.get()),
    };
    let mut chunks = Vec::new();
    for row in 0..count {
        let src = memory.strided_start(dma_space, source, row, operation.source_stride)?;
        let dst =
            memory.strided_start(dma_space, destination, row, operation.destination_stride)?;
        chunks.push(NativeCopyChunk {
            src,
            dst,
            len: operation.row_bytes,
        });
    }
    Ok(chunks)
}
