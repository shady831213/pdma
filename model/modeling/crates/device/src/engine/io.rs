mod frontend;
mod memory;
mod semantic;

use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use crate::engine::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    Command, CompletionRecord, CompletionRingId, EventRecord, QueueDirectoryEntry, QueueId,
};

pub(super) use frontend::QueueFrontend;
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

pub(crate) trait FrontendBehavior: std::fmt::Debug + Send + Sync {
    fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>>;

    fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>>;

    fn consume(&mut self, control: &mut QueueControlState, queue: QueueId) -> ModelResult<()>;

    fn has_pending(&self, control: &QueueControlState, queue: QueueId) -> bool;

    fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut QueueControlState,
        queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome;

    fn stage_execution_fault(&mut self, event: EventRecord) -> ModelResult<ModelError>;

    fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut QueueControlState,
        trace: &mut TraceSink,
    ) -> PublicationOutcome;

    fn completion_blocked(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool;

    fn fault_blocked(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        compat_publication_slots: usize,
    ) -> bool;

    fn records_pending(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool>;
}

pub(super) fn semantic(
    command_depth: usize,
    completion_depth: usize,
) -> ModelResult<QueueFrontend> {
    semantic::SemanticFrontend::new(command_depth, completion_depth).map(QueueFrontend::Semantic)
}

pub(super) const fn exact() -> QueueFrontend {
    QueueFrontend::Exact(crate::abi::ring::frontend::ExactFrontend::new())
}

pub(super) const fn rejected_compat_submission(queue: QueueId) -> ModelError {
    ModelError::UnknownQueue(queue)
}
