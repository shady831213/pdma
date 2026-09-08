use crate::ModelError;
use crate::operation::DecodedCommand;
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueId};

#[derive(Debug, Clone)]
pub(crate) enum NormalizedFetch {
    Command {
        command: DecodedCommand,
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
    Exact(Option<(QueueId, CommandId)>),
}
