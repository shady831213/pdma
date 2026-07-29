use super::NativeExecution;
use super::io::QueueMemoryResolver;
use super::retirement::validate_success_completion_route;
use super::retirement::{FaultRetirement, SuccessRetirement, publish_completion, publish_fault};
use crate::runner::backend::ModelStepStatus;
use pdma_model_contract::desc::{CopyCommand, StridedCopyCommand};
use pdma_model_contract::dma::DmaRangeStart;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::ids::QueueId;
use pdma_model_contract::trace::TraceOperation;

#[derive(Debug, Clone, Copy)]
struct NativeCopyChunk {
    src: DmaRangeStart,
    dst: DmaRangeStart,
    len: usize,
}

pub(super) fn retire_copy(
    execution: NativeExecution<'_>,
    queue_id: QueueId,
    command: CopyCommand,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let request = command.request();
    let memory = device.memory_resolver(queue_id, command.command_id())?;
    let bytes = match memory.read(dma_space, request.src_start(), request.len()) {
        Ok(bytes) => bytes,
        Err(fault) => return publish_fault(device, dma_space, bus_control, fault),
    };
    if let Err(fault) = memory.validate(dma_space, request.dst_start(), request.len()) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    validate_success_completion_route(
        device,
        dma_space,
        bus_control,
        queue_id,
        command.completion_ring_id(),
    )?;
    if let Err(fault) = memory.write(dma_space, request.dst_start(), &bytes) {
        return publish_fault(device, dma_space, bus_control, fault);
    }
    publish_completion(
        device,
        dma_space,
        bus_control,
        SuccessRetirement {
            queue: queue_id,
            command: command.command_id(),
            completion_ring: command.completion_ring_id(),
            operation: TraceOperation::Copy,
        },
    )
}

pub(super) fn retire_strided_copy(
    execution: NativeExecution<'_>,
    queue_id: QueueId,
    command: StridedCopyCommand,
) -> ModelResult<ModelStepStatus> {
    let NativeExecution {
        device,
        dma_space,
        bus_control,
    } = execution;
    let memory = device.memory_resolver(queue_id, command.command_id())?;
    let chunks = match strided_chunks(dma_space, memory, command) {
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
        queue_id,
        command.completion_ring_id(),
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
            queue: queue_id,
            command: command.command_id(),
            completion_ring: command.completion_ring_id(),
            operation: TraceOperation::StridedCopy,
        },
    )
}

fn strided_chunks(
    dma_space: &crate::dma_space::DmaSpace,
    memory: QueueMemoryResolver,
    command: StridedCopyCommand,
) -> Result<Vec<NativeCopyChunk>, FaultRetirement> {
    let request = command.request();
    let mut chunks = Vec::new();
    for row in 0..request.rows() {
        let src =
            memory.strided_start(dma_space, request.src_start(), row, request.src_stride())?;
        let dst =
            memory.strided_start(dma_space, request.dst_start(), row, request.dst_stride())?;
        chunks.push(NativeCopyChunk {
            src,
            dst,
            len: request.row_bytes(),
        });
    }
    Ok(chunks)
}
