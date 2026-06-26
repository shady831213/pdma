//! Behavioral scheduler lowering for the first PDMA model slice.

use crate::ModelResult;
use crate::desc::{
    BroadcastCopyCommand, CopyChunk, CopyCommand, ExternalCommand, ScheduledCopy,
    StridedCopyCommand,
};

/// Stateless behavioral scheduler for linear copy commands.
#[derive(Debug, Default, Clone, Copy)]
pub struct Scheduler;

impl Scheduler {
    /// Lowers one external command into internal scheduled work.
    pub fn lower(command: ExternalCommand) -> ModelResult<ScheduledCopy> {
        match command {
            ExternalCommand::Copy(copy) => Self::lower_copy(copy),
            ExternalCommand::StridedCopy(strided) => Self::lower_strided_copy(strided),
            ExternalCommand::BroadcastCopy(broadcast) => Self::lower_broadcast_copy(&broadcast),
        }
    }

    /// Lowers one external copy command into one internal copy descriptor.
    pub fn lower_copy(command: CopyCommand) -> ModelResult<ScheduledCopy> {
        Ok(ScheduledCopy::new(
            command.queue_id(),
            command.command_id(),
            command.completion_ring_id(),
            vec![CopyChunk::new(
                command.request().src(),
                command.request().dst(),
                0,
                0,
                command.request().len(),
            )],
        ))
    }

    /// Lowers one external strided copy command into row-level copy chunks.
    pub fn lower_strided_copy(command: StridedCopyCommand) -> ModelResult<ScheduledCopy> {
        let request = command.request();
        let mut chunks = Vec::new();
        for row in 0..request.rows() {
            let src_offset = row
                .checked_mul(request.src_stride())
                .ok_or(crate::ModelError::RangeOverflow)?;
            let dst_offset = row
                .checked_mul(request.dst_stride())
                .ok_or(crate::ModelError::RangeOverflow)?;
            chunks.push(CopyChunk::new(
                request.src(),
                request.dst(),
                src_offset,
                dst_offset,
                request.row_bytes(),
            ));
        }
        Ok(ScheduledCopy::new(
            command.queue_id(),
            command.command_id(),
            command.completion_ring_id(),
            chunks,
        ))
    }

    /// Lowers one external broadcast copy command into destination-level copy chunks.
    pub fn lower_broadcast_copy(command: &BroadcastCopyCommand) -> ModelResult<ScheduledCopy> {
        let request = command.request();
        let chunks = request
            .destinations()
            .iter()
            .map(|destination| {
                CopyChunk::new(
                    request.src(),
                    destination.dst(),
                    request.src_offset(),
                    destination.dst_offset(),
                    request.len(),
                )
            })
            .collect();
        Ok(ScheduledCopy::new(
            command.queue_id(),
            command.command_id(),
            command.completion_ring_id(),
            chunks,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Scheduler;
    use crate::desc::{
        BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange, CopyChunk,
        CopyCommand, CopyRequest,
    };
    use crate::ids::{BufferId, CommandId, CompletionRingId, QueueId};

    #[test]
    fn lowers_external_copy_to_internal_copy_work() {
        // Given: an external copy command.
        let request = CopyRequest::new(BufferId::new(1), BufferId::new(2), 64);
        let command = CopyCommand::new(
            CommandId::new(3),
            QueueId::new(4),
            CompletionRingId::new(5),
            request,
        );

        // When: the behavioral scheduler lowers the command.
        let work = Scheduler::lower_copy(command).expect("copy lowering cannot fail");

        // Then: the internal work preserves routing and copy metadata.
        assert_eq!(work.command_id(), CommandId::new(3));
        assert_eq!(work.queue_id(), QueueId::new(4));
        assert_eq!(work.completion_ring_id(), CompletionRingId::new(5));
        assert_eq!(
            work.chunks(),
            &[CopyChunk::new(
                request.src(),
                request.dst(),
                0,
                0,
                request.len()
            )]
        );
    }

    #[test]
    fn lowers_external_broadcast_copy_to_internal_copy_work() {
        // Given: an external broadcast command with one source range and two destinations.
        let request = BroadcastCopyRequest::new(
            BufferId::new(1),
            8,
            16,
            vec![
                BroadcastDestinationRange::new(BufferId::new(2), 0),
                BroadcastDestinationRange::new(BufferId::new(3), 32),
            ],
        );
        let command = BroadcastCopyCommand::new(
            CommandId::new(4),
            QueueId::new(5),
            CompletionRingId::new(6),
            request,
        );

        // When: the behavioral scheduler lowers the broadcast command.
        let work = Scheduler::lower_broadcast_copy(&command).expect("broadcast lowering succeeds");

        // Then: the internal work fans out one chunk per destination.
        assert_eq!(work.command_id(), CommandId::new(4));
        assert_eq!(work.queue_id(), QueueId::new(5));
        assert_eq!(work.completion_ring_id(), CompletionRingId::new(6));
        assert_eq!(
            work.chunks(),
            &[
                CopyChunk::new(BufferId::new(1), BufferId::new(2), 8, 0, 16),
                CopyChunk::new(BufferId::new(1), BufferId::new(3), 8, 32, 16),
            ]
        );
    }
}
