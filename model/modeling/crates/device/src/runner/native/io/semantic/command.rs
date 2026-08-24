use super::super::super::execution::{
    BroadcastCopyOperation, CopyOperation, ExecutionCommand, ExecutionOperation,
    ResolvedStridedRows, StridedCopyOperation, VectorAddOperation,
};
use pdma_model_contract::semantic::desc::ExternalCommand;
use pdma_model_contract::semantic::{DmaRangeStart, TraceOperation};
use std::num::NonZeroUsize;

pub(in crate::runner::native::io) fn normalize(command: ExternalCommand) -> ExecutionCommand {
    match command {
        ExternalCommand::Copy(command) => {
            let request = command.request();
            ExecutionCommand::new(
                command.command_id(),
                command.completion_ring_id(),
                ExecutionOperation::Copy(CopyOperation {
                    source: request.src_start(),
                    destination: request.dst_start(),
                    length: request.len(),
                }),
            )
        }
        ExternalCommand::StridedCopy(command) => {
            let request = command.request();
            let rows =
                NonZeroUsize::new(request.rows()).map_or(ResolvedStridedRows::Empty, |count| {
                    ResolvedStridedRows::NonEmpty {
                        source: request.src_start(),
                        destination: request.dst_start(),
                        count,
                    }
                });
            ExecutionCommand::new(
                command.command_id(),
                command.completion_ring_id(),
                ExecutionOperation::StridedCopy(StridedCopyOperation {
                    rows,
                    row_bytes: request.row_bytes(),
                    source_stride: request.src_stride(),
                    destination_stride: request.dst_stride(),
                }),
            )
        }
        ExternalCommand::BroadcastCopy(command) => {
            let request = command.request();
            let destinations = request
                .destinations()
                .iter()
                .map(|destination| DmaRangeStart::new(destination.dst(), destination.dst_offset()))
                .collect();
            ExecutionCommand::new(
                command.command_id(),
                command.completion_ring_id(),
                ExecutionOperation::BroadcastCopy(BroadcastCopyOperation {
                    source: DmaRangeStart::new(request.src(), request.src_offset()),
                    destinations,
                    length: request.len(),
                }),
            )
        }
        ExternalCommand::VectorAdd(command) => {
            let request = command.request();
            ExecutionCommand::new(
                command.command_id(),
                command.completion_ring_id(),
                ExecutionOperation::VectorAdd(VectorAddOperation {
                    source_a: DmaRangeStart::new(request.src_a(), request.src_a_offset()),
                    source_b: DmaRangeStart::new(request.src_b(), request.src_b_offset()),
                    destination: DmaRangeStart::new(request.dst(), request.dst_offset()),
                    length: request.len(),
                }),
            )
        }
    }
}

pub(super) const fn identity(
    command: &ExecutionCommand,
) -> (pdma_model_contract::semantic::CommandId, TraceOperation) {
    (command.command_id(), command.trace_operation())
}
