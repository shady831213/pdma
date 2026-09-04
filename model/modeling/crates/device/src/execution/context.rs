use pdma_model_contract::semantic::{
    CommandId, CompletionRingId, MemoryFault, QueueId, TraceOperation,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandRoute {
    pub(crate) queue: QueueId,
    pub(crate) command: CommandId,
    pub(crate) completion_ring: CompletionRingId,
    completion_valid: bool,
}

impl CommandRoute {
    pub(crate) const fn new(
        queue: QueueId,
        command: CommandId,
        completion_ring: CompletionRingId,
        completion_valid: bool,
    ) -> Self {
        Self {
            queue,
            command,
            completion_ring,
            completion_valid,
        }
    }

    pub(super) const fn completion_is_valid(self) -> bool {
        self.completion_valid
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SuccessfulExecution {
    pub(crate) route: CommandRoute,
    pub(crate) operation: TraceOperation,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExecutionFault {
    pub(crate) queue: QueueId,
    pub(crate) command: CommandId,
    pub(crate) fault: MemoryFault,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExecutionOutcome {
    Success(SuccessfulExecution),
    Fault(ExecutionFault),
    InvalidCompletion(CommandRoute),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RetirableExecution {
    Success(SuccessfulExecution),
    Fault(ExecutionFault),
}

pub(super) const fn success(route: CommandRoute, operation: TraceOperation) -> ExecutionOutcome {
    ExecutionOutcome::Success(SuccessfulExecution { route, operation })
}
