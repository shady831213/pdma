pub(super) mod exact;
mod frontend;
mod memory;
mod semantic;

use crate::dma_space::PdmaMemoryMappings;
use crate::runner::abi::bus::control::BusControlState;
use crate::runner::execution::command::ExecutionCommand;
use crate::runner::execution::retirement::FaultRetirement;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{CompletionRecord, QueueDirectoryEntry, QueueId};

pub(super) use frontend::QueueFrontend;
pub(in crate::runner::execution) use memory::QueueMemoryResolver;

#[derive(Debug, Clone)]
pub(super) enum NormalizedFetch {
    Command(ExecutionCommand),
    Fault(NormalizedFault),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct NormalizedFault {
    pub(super) queue: QueueId,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PublicationOutcome {
    Published,
    Retry,
}

pub(super) trait FrontendBehavior: std::fmt::Debug + Send + Sync {
    fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>>;

    fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>>;

    fn consume(&mut self, control: &mut BusControlState, queue: QueueId) -> ModelResult<()>;

    fn has_pending(&self, control: &BusControlState, queue: QueueId) -> bool;

    fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome;

    fn stage_execution_fault(&mut self, fault: FaultRetirement) -> ModelResult<ModelError>;

    fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        trace: &mut TraceSink,
    ) -> PublicationOutcome;

    fn completion_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool;

    fn fault_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        compat_publication_slots: usize,
    ) -> bool;

    fn records_pending(
        &self,
        control: &BusControlState,
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
    QueueFrontend::Exact(exact::ExactFrontend::new())
}

pub(super) const fn rejected_compat_submission(queue: QueueId) -> ModelError {
    ModelError::UnknownQueue(queue)
}
