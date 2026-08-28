use super::super::super::command::{
    BroadcastCopyOperation, CopyOperation, ExecutionCommand, ExecutionOperation,
    ResolvedStridedRows, StridedCopyOperation, VectorAddOperation,
};
use pdma_model_contract::semantic::{Command, CommandOperation, CompletionRingId, DmaRangeStart};
use std::num::NonZeroUsize;

pub(in crate::runner::execution::io) fn normalize(
    completion_ring: CompletionRingId,
    command: Command,
) -> ExecutionCommand {
    let command_id = command.command_id();
    match command.into_operation() {
        CommandOperation::Copy(request) => ExecutionCommand::new(
            command_id,
            completion_ring,
            ExecutionOperation::Copy(CopyOperation {
                source: request.src_start(),
                destination: request.dst_start(),
                length: request.len(),
            }),
        ),
        CommandOperation::StridedCopy(request) => {
            let rows =
                NonZeroUsize::new(request.rows()).map_or(ResolvedStridedRows::Empty, |count| {
                    ResolvedStridedRows::NonEmpty {
                        source: request.src_start(),
                        destination: request.dst_start(),
                        count,
                    }
                });
            ExecutionCommand::new(
                command_id,
                completion_ring,
                ExecutionOperation::StridedCopy(StridedCopyOperation {
                    rows,
                    row_bytes: request.row_bytes(),
                    source_stride: request.src_stride(),
                    destination_stride: request.dst_stride(),
                }),
            )
        }
        CommandOperation::BroadcastCopy(request) => {
            let destinations = request
                .destinations()
                .iter()
                .map(|destination| DmaRangeStart::new(destination.dst(), destination.dst_offset()))
                .collect();
            ExecutionCommand::new(
                command_id,
                completion_ring,
                ExecutionOperation::BroadcastCopy(BroadcastCopyOperation {
                    source: DmaRangeStart::new(request.src(), request.src_offset()),
                    destinations,
                    length: request.len(),
                }),
            )
        }
        CommandOperation::VectorAdd(request) => ExecutionCommand::new(
            command_id,
            completion_ring,
            ExecutionOperation::VectorAdd(VectorAddOperation {
                source_a: DmaRangeStart::new(request.src_a(), request.src_a_offset()),
                source_b: DmaRangeStart::new(request.src_b(), request.src_b_offset()),
                destination: DmaRangeStart::new(request.dst(), request.dst_offset()),
                length: request.len(),
            }),
        ),
    }
}
