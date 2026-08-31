use super::semantic::SemanticFrontend;
use super::{FrontendBehavior, NormalizedFetch, PublicationOutcome, rejected_compat_submission};
use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use crate::abi::ring::frontend::ExactFrontend;
use crate::engine::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    Command, CommandId, CompletionRecord, EventRecord, QueueDirectoryEntry, QueueId, TraceOperation,
};

#[derive(Debug)]
pub(in crate::engine) enum QueueFrontend {
    Semantic(SemanticFrontend),
    Exact(ExactFrontend),
}

impl QueueFrontend {
    pub(in crate::engine) const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    fn behavior(&self) -> &dyn FrontendBehavior {
        match self {
            Self::Semantic(frontend) => frontend,
            Self::Exact(frontend) => frontend,
        }
    }

    fn behavior_mut(&mut self) -> &mut dyn FrontendBehavior {
        match self {
            Self::Semantic(frontend) => frontend,
            Self::Exact(frontend) => frontend,
        }
    }

    pub(in crate::engine) fn submit_semantic(
        &mut self,
        queue: QueueId,
        command: Command,
    ) -> ModelResult<(CommandId, TraceOperation)> {
        match self {
            Self::Semantic(frontend) => frontend.submit(command),
            Self::Exact(_) => Err(rejected_compat_submission(queue)),
        }
    }

    pub(in crate::engine) fn poll_semantic_completion(&mut self) -> Option<CompletionRecord> {
        match self {
            Self::Semantic(frontend) => frontend.poll_completion(),
            Self::Exact(_) => None,
        }
    }

    pub(in crate::engine) fn poll_semantic_event(&mut self, queue: QueueId) -> Option<EventRecord> {
        match self {
            Self::Semantic(frontend) => frontend.poll_event(queue),
            Self::Exact(_) => None,
        }
    }

    pub(in crate::engine) const fn pending_exact_fault(
        &self,
    ) -> Option<crate::abi::ring::PendingDmaResult> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(frontend) => frontend.pending_result(),
        }
    }

    pub(in crate::engine) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        self.behavior().probe(memory, control, entry)
    }

    pub(in crate::engine) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        self.behavior_mut().fetch(memory, control, entry)
    }

    pub(in crate::engine) fn consume(
        &mut self,
        control: &mut QueueControlState,
        queue: QueueId,
    ) -> ModelResult<()> {
        self.behavior_mut().consume(control, queue)
    }

    pub(in crate::engine) fn has_pending(
        &self,
        control: &QueueControlState,
        queue: QueueId,
    ) -> bool {
        self.behavior().has_pending(control, queue)
    }

    pub(in crate::engine) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut QueueControlState,
        queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        self.behavior_mut()
            .publish_completion(memory, control, queue, record)
    }

    pub(in crate::engine) fn stage_execution_fault(
        &mut self,
        event: EventRecord,
    ) -> ModelResult<ModelError> {
        self.behavior_mut().stage_execution_fault(event)
    }

    pub(in crate::engine) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut QueueControlState,
        trace: &mut TraceSink,
    ) -> PublicationOutcome {
        self.behavior_mut().publish_fault(memory, control, trace)
    }

    pub(in crate::engine) fn completion_blocked(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        self.behavior()
            .completion_blocked(control, queue, publication_slots)
    }

    pub(in crate::engine) fn fault_blocked(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        self.behavior()
            .fault_blocked(control, queue, publication_slots)
    }

    pub(in crate::engine) fn records_pending(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool> {
        self.behavior().records_pending(control, queue, completion)
    }
}
