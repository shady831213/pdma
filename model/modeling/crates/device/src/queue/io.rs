use crate::ModelError;
use pdma_model_contract::semantic::{Command, CommandId, CompletionRingId, EventRecord, QueueId};

#[derive(Debug, Clone)]
pub(crate) enum NormalizedFetch {
    Command {
        command: Command,
        completion_ring: CompletionRingId,
    },
    Fault(NormalizedFault),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct NormalizedFault {
    pub(crate) queue: QueueId,
    pub(crate) source: ModelError,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PublicationOutcome {
    Published,
    Retry,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FaultPublication {
    Retry,
    Semantic(EventRecord),
    Exact(Option<(QueueId, CommandId)>),
}
