use super::ModelStepStatus;
use crate::queue::io::NormalizedFault;
use pdma_model_contract::semantic::{
    CommandId, CommandOperation, CompletionRingId, MemoryFault, QueueId, TraceOperation,
};

#[derive(Debug)]
pub(super) struct FetchedCommand {
    pub(super) route: CommandRoute,
    pub(super) operation: CommandOperation,
}

#[derive(Debug)]
pub(super) enum FetchedWork {
    Empty(ModelStepStatus),
    FrontendFault(NormalizedFault),
    Command(FetchedCommand),
}

#[derive(Debug)]
pub(super) enum RetirementWork {
    Empty(ModelStepStatus),
    FrontendFault(NormalizedFault),
    Executed(ExecutionOutcome),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CommandRoute {
    pub(super) queue: QueueId,
    pub(super) command: CommandId,
    pub(super) completion_ring: CompletionRingId,
    completion_valid: bool,
}

impl CommandRoute {
    pub(super) const fn new(
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
pub(super) struct SuccessfulExecution {
    pub(super) route: CommandRoute,
    pub(super) operation: TraceOperation,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ExecutionFault {
    pub(super) queue: QueueId,
    pub(super) command: CommandId,
    pub(super) fault: MemoryFault,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ExecutionOutcome {
    Success(SuccessfulExecution),
    Fault(ExecutionFault),
    InvalidCompletion(CommandRoute),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RetirableExecution {
    Success(SuccessfulExecution),
    Fault(ExecutionFault),
}
