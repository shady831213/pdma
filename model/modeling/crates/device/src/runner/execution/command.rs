use pdma_model_contract::semantic::{
    CommandId, CompletionRingId, DmaRangeStart, QueueId, TraceOperation,
};
use std::num::NonZeroUsize;

#[derive(Debug, Clone)]
pub(super) struct ExecutionCommand {
    command_id: CommandId,
    completion_ring_id: CompletionRingId,
    operation: ExecutionOperation,
}

impl ExecutionCommand {
    pub(super) const fn new(
        command_id: CommandId,
        completion_ring_id: CompletionRingId,
        operation: ExecutionOperation,
    ) -> Self {
        Self {
            command_id,
            completion_ring_id,
            operation,
        }
    }

    pub(super) const fn command_id(&self) -> CommandId {
        self.command_id
    }

    pub(super) const fn trace_operation(&self) -> TraceOperation {
        match self.operation {
            ExecutionOperation::Copy(_) => TraceOperation::Copy,
            ExecutionOperation::StridedCopy(_) => TraceOperation::StridedCopy,
            ExecutionOperation::BroadcastCopy(_) => TraceOperation::BroadcastCopy,
            ExecutionOperation::VectorAdd(_) => TraceOperation::VectorAdd,
        }
    }

    pub(super) const fn operation(&self) -> &ExecutionOperation {
        &self.operation
    }

    pub(super) fn into_operation(self) -> ExecutionOperation {
        self.operation
    }

    pub(super) const fn route(&self, queue_id: QueueId) -> ExecutionRoute {
        ExecutionRoute {
            queue: queue_id,
            command: self.command_id,
            completion_ring: self.completion_ring_id,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum ExecutionOperation {
    Copy(CopyOperation),
    StridedCopy(StridedCopyOperation),
    BroadcastCopy(BroadcastCopyOperation),
    VectorAdd(VectorAddOperation),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CopyOperation {
    pub(super) source: DmaRangeStart,
    pub(super) destination: DmaRangeStart,
    pub(super) length: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StridedCopyOperation {
    pub(super) rows: ResolvedStridedRows,
    pub(super) row_bytes: usize,
    pub(super) source_stride: usize,
    pub(super) destination_stride: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ResolvedStridedRows {
    Empty,
    NonEmpty {
        source: DmaRangeStart,
        destination: DmaRangeStart,
        count: NonZeroUsize,
    },
}

impl ResolvedStridedRows {
    pub(super) const fn count(self) -> usize {
        match self {
            Self::Empty => 0,
            Self::NonEmpty { count, .. } => count.get(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct BroadcastCopyOperation {
    pub(super) source: DmaRangeStart,
    pub(super) destinations: Vec<DmaRangeStart>,
    pub(super) length: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct VectorAddOperation {
    pub(super) source_a: DmaRangeStart,
    pub(super) source_b: DmaRangeStart,
    pub(super) destination: DmaRangeStart,
    pub(super) length: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ExecutionRoute {
    pub(super) queue: QueueId,
    pub(super) command: CommandId,
    pub(super) completion_ring: CompletionRingId,
}
