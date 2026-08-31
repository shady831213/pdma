mod memory;
pub mod semantic;

use crate::ModelError;
use pdma_model_contract::semantic::{Command, CompletionRingId, QueueId};

pub(in crate::engine) use memory::QueueMemoryResolver;

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
