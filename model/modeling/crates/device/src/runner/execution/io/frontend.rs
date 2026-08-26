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

    pub(in crate::runner::execution) fn pending_exact_fault_bytes(&self) -> Option<[u8; 64]> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(frontend) => frontend.pending_fault_bytes(),
        }
    }

    pub(in crate::runner::execution) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(frontend) => frontend.probe(memory, control, entry),
            Self::Exact(frontend) => frontend.probe(memory, control, entry),
        }
    }

    pub(in crate::runner::execution) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &BusControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(frontend) => frontend.fetch(memory, control, entry),
            Self::Exact(frontend) => frontend.fetch(memory, control, entry),
        }
    }

    pub(in crate::runner::execution) fn consume(
        &mut self,
        control: &mut BusControlState,
        queue: QueueId,
    ) -> ModelResult<()> {
        match self {
            Self::Semantic(frontend) => frontend.consume(control, queue),
            Self::Exact(frontend) => frontend.consume(control, queue),
        }
    }

    pub(in crate::runner::execution) fn has_pending(
        &self,
        control: &BusControlState,
        queue: QueueId,
    ) -> bool {
        match self {
            Self::Semantic(frontend) => frontend.has_pending(control, queue),
            Self::Exact(frontend) => frontend.has_pending(control, queue),
        }
    }

    pub(in crate::runner::execution) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        match self {
            Self::Semantic(frontend) => frontend.publish_completion(memory, control, queue, record),
            Self::Exact(frontend) => frontend.publish_completion(memory, control, queue, record),
        }
    }

    pub(in crate::runner::execution) fn stage_execution_fault(
        &mut self,
        fault: FaultRetirement,
    ) -> ModelResult<ModelError> {
        match self {
            Self::Semantic(frontend) => frontend.stage_execution_fault(fault),
            Self::Exact(frontend) => frontend.stage_execution_fault(fault),
        }
    }

    pub(in crate::runner::execution) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
        trace: &mut TraceSink,
    ) -> PublicationOutcome {
        match self {
            Self::Semantic(frontend) => frontend.publish_fault(memory, control, trace),
            Self::Exact(frontend) => frontend.publish_fault(memory, control, trace),
        }
    }

    pub(in crate::runner::execution) fn completion_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        match self {
            Self::Semantic(frontend) => {
                frontend.completion_blocked(control, queue, publication_slots)
            }
            Self::Exact(frontend) => frontend.completion_blocked(control, queue, publication_slots),
        }
    }

    pub(in crate::runner::execution) fn fault_blocked(
        &self,
        control: &BusControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        match self {
            Self::Semantic(frontend) => frontend.fault_blocked(control, queue, publication_slots),
            Self::Exact(frontend) => frontend.fault_blocked(control, queue, publication_slots),
        }
    }

    pub(in crate::runner::execution) fn records_pending(
        &self,
        control: &BusControlState,
        queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool> {
        match self {
            Self::Semantic(frontend) => frontend.records_pending(control, queue, completion),
            Self::Exact(frontend) => frontend.records_pending(control, queue, completion),
        }
    }
}
