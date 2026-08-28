use super::exact::ExactFrontend;
use super::semantic::SemanticFrontend;
use super::{FrontendBehavior, NormalizedFetch, PublicationOutcome, rejected_compat_submission};
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::abi::bus::control::BusControlState;
use crate::runner::execution::retirement::FaultRetirement;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    CommandId, CompletionRecord, EventRecord, ExternalCommand, QueueDirectoryEntry, QueueId,
    TraceOperation,
};

#[derive(Debug)]
pub(in crate::runner::execution) enum QueueFrontend {
    Semantic(SemanticFrontend),
    Exact(ExactFrontend),
}

impl QueueFrontend {
    pub(in crate::runner::execution) const fn is_exact(&self) -> bool {
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

    pub(in crate::runner::execution) fn submit_semantic(
        &mut self,
        queue: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<(CommandId, TraceOperation)> {
        match self {
            Self::Semantic(frontend) => frontend.submit(command),
            Self::Exact(_) => Err(rejected_compat_submission(queue)),
        }
    }

    pub(in crate::runner::execution) fn poll_semantic_completion(
        &mut self,
    ) -> Option<CompletionRecord> {
        match self {
            Self::Semantic(frontend) => frontend.poll_completion(),
            Self::Exact(_) => None,
        }
    }

    pub(in crate::runner::execution) fn poll_semantic_event(
        &mut self,
        queue: QueueId,
    ) -> Option<EventRecord> {
        match self {
            Self::Semantic(frontend) => frontend.poll_event(queue),
            Self::Exact(_) => None,
        }
    }

    pub(in crate::runner::execution) const fn pending_exact_fault(
        &self,
    ) -> Option<pdma_model_contract::abi::AbiEvent> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(frontend) => frontend.pending_fault(),
        }
    }

    pub(in crate::runner::execution) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        self.behavior().probe(memory, control, entry)
    }

    pub(in crate::runner::execution) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        self.behavior_mut().fetch(memory, control, entry)
    }

    pub(in crate::runner::execution) fn consume(
        &mut self,
        control: &mut BusControlState,
        queue: QueueId,
    ) -> ModelResult<()> {
        self.behavior_mut().consume(control, queue)
    }

    pub(in crate::runner::execution) fn has_pending(
        &self,
        control: &BusControlState,
        queue: QueueId,
    ) -> bool {
        self.behavior().has_pending(control, queue)
    }

    pub(in crate::runner::execution) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        self.behavior_mut()
            .publish_completion(memory, control, queue, record)
    }

    pub(in crate::runner::execution) fn stage_execution_fault(
        &mut self,
        fault: FaultRetirement,
    ) -> ModelResult<ModelError> {
        self.behavior_mut().stage_execution_fault(fault)
    }

    pub(in crate::runner::execution) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        trace: &mut TraceSink,
    ) -> PublicationOutcome {
        self.behavior_mut().publish_fault(memory, control, trace)
    }

    pub(in crate::runner::execution) fn completion_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        self.behavior()
            .completion_blocked(control, queue, publication_slots)
    }

    pub(in crate::runner::execution) fn fault_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        self.behavior()
            .fault_blocked(control, queue, publication_slots)
    }

    pub(in crate::runner::execution) fn records_pending(
        &self,
        control: &BusControlState,
        queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool> {
        self.behavior().records_pending(control, queue, completion)
    }
}
