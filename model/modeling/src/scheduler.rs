//! Behavioral scheduler lowering for the first PDMA model slice.

use crate::desc::{CopyCommand, ExternalCommand, ScheduledCopy};

/// Stateless behavioral scheduler for linear copy commands.
#[derive(Debug, Default, Clone, Copy)]
pub struct Scheduler;

impl Scheduler {
    /// Lowers one external command into internal scheduled work.
    pub const fn lower(command: ExternalCommand) -> ScheduledCopy {
        match command {
            ExternalCommand::Copy(copy) => Self::lower_copy(copy),
        }
    }

    /// Lowers one external copy command into one internal copy descriptor.
    pub const fn lower_copy(command: CopyCommand) -> ScheduledCopy {
        ScheduledCopy::new(command)
    }
}

#[cfg(test)]
mod tests {
    use super::Scheduler;
    use crate::desc::{CopyCommand, CopyRequest};
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
        let work = Scheduler::lower_copy(command);

        // Then: the internal work preserves routing and copy metadata.
        assert_eq!(work.command().command_id(), CommandId::new(3));
        assert_eq!(work.command().queue_id(), QueueId::new(4));
        assert_eq!(
            work.command().completion_ring_id(),
            CompletionRingId::new(5)
        );
        assert_eq!(work.request(), request);
    }
}
