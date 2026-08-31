use crate::ModelError;
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueId, TraceOperation};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SuccessRetirement {
    pub(crate) queue: QueueId,
    pub(crate) command: CommandId,
    pub(crate) completion_ring: CompletionRingId,
    pub(crate) operation: TraceOperation,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PendingRetirement {
    Completion(SuccessRetirement),
    Fault {
        queue: QueueId,
        source: Option<ModelError>,
    },
}

#[derive(Debug, Default)]
pub(crate) struct RetirementUnit {
    pending: Option<PendingRetirement>,
}

impl RetirementUnit {
    pub(crate) const fn pending(&self) -> Option<PendingRetirement> {
        self.pending
    }

    pub(crate) const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) const fn hold(&mut self, pending: PendingRetirement) {
        self.pending = Some(pending);
    }

    pub(crate) const fn complete(&mut self) {
        self.pending = None;
    }

    pub(crate) const fn reset(&mut self) {
        self.complete();
    }
}
