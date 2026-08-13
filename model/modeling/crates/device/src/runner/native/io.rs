pub(super) mod exact;
mod memory;
mod semantic;

use crate::dma_space::PdmaMemoryMappings;
use crate::runner::bus::control::BusControlState;
use crate::runner::native::retirement::FaultRetirement;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::{CompletionRecord, EventRecord, QueueDirectoryEntry, QueueId};

pub(in crate::runner::native) use memory::QueueMemoryResolver;

#[derive(Debug, Clone)]
pub(super) enum NormalizedFetch {
    Command(ExternalCommand),
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

pub(super) trait QueueFrontend: std::fmt::Debug + Send + Sync {
    fn submit_compat(&mut self, queue: QueueId, command: ExternalCommand) -> ModelResult<()>;

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

    fn pending_fault_bytes(&self) -> Option<[u8; 64]>;

    fn poll_compat_completion(&mut self) -> Option<CompletionRecord>;

    fn poll_compat_event(&mut self, queue: QueueId) -> Option<EventRecord>;

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

    fn accepts_exact_control(&self) -> bool;
}

pub(super) fn semantic(
    command_depth: usize,
    completion_depth: usize,
) -> ModelResult<Box<dyn QueueFrontend>> {
    semantic::SemanticFrontend::new(command_depth, completion_depth)
        .map(|frontend| Box::new(frontend) as Box<dyn QueueFrontend>)
}

pub(super) fn exact() -> Box<dyn QueueFrontend> {
    Box::new(exact::ExactFrontend::new())
}

pub(super) const fn rejected_compat_submission(queue: QueueId) -> ModelError {
    ModelError::UnknownQueue(queue)
}
