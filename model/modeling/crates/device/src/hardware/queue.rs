use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use crate::abi::ring::frontend::ExactFrontend;
use crate::engine::io::semantic::SemanticFrontend;
use crate::engine::io::{NormalizedFetch, PublicationOutcome};
use crate::engine::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    Command, CommandId, CompletionRecord, CompletionRingId, EventRecord, QueueConfig,
    QueueDirectoryEntry, QueueId, TraceOperation,
};

#[derive(Debug)]
pub(crate) struct PdmaQueue {
    pub(crate) entry: QueueDirectoryEntry,
    pub(crate) io: QueueIo,
}

impl PdmaQueue {
    pub(crate) fn semantic(
        queue: QueueId,
        completion: CompletionRingId,
        config: QueueConfig,
    ) -> ModelResult<Self> {
        Ok(Self {
            entry: QueueDirectoryEntry::new(queue, completion, config),
            io: QueueIo::Semantic(SemanticFrontend::new(
                config.command_depth(),
                config.completion_depth(),
            )?),
        })
    }

    pub(crate) const fn exact(
        queue: QueueId,
        completion: CompletionRingId,
        config: QueueConfig,
    ) -> Self {
        Self {
            entry: QueueDirectoryEntry::new(queue, completion, config),
            io: QueueIo::Exact(ExactFrontend::new()),
        }
    }
}

#[derive(Debug)]
pub(crate) enum QueueIo {
    Semantic(SemanticFrontend),
    Exact(ExactFrontend),
}

impl QueueIo {
    pub(crate) const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    pub(crate) fn submit_semantic(
        &mut self,
        queue: QueueId,
        command: Command,
    ) -> ModelResult<(CommandId, TraceOperation)> {
        match self {
            Self::Semantic(io) => io.submit(command),
            Self::Exact(_) => Err(ModelError::UnknownQueue(queue)),
        }
    }

    pub(crate) fn poll_semantic_completion(&mut self) -> Option<CompletionRecord> {
        match self {
            Self::Semantic(io) => io.poll_completion(),
            Self::Exact(_) => None,
        }
    }

    pub(crate) fn poll_semantic_event(&mut self, queue: QueueId) -> Option<EventRecord> {
        match self {
            Self::Semantic(io) => io.poll_event(queue),
            Self::Exact(_) => None,
        }
    }

    pub(crate) const fn pending_exact_fault(&self) -> Option<crate::abi::ring::PendingDmaResult> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(io) => io.pending_result(),
        }
    }

    pub(crate) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(io) => Ok(io.probe(memory, control, entry)),
            Self::Exact(_) => ExactFrontend::probe(memory, control, entry),
        }
    }

    pub(crate) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(io) => Ok(io.fetch(memory, control, entry)),
            Self::Exact(io) => io.fetch(memory, control, entry),
        }
    }

    pub(crate) fn consume(
        &mut self,
        control: &mut QueueControlState,
        queue: QueueId,
    ) -> ModelResult<()> {
        match self {
            Self::Semantic(io) => {
                io.consume(control, queue);
                Ok(())
            }
            Self::Exact(_) => ExactFrontend::consume(control, queue),
        }
    }

    pub(crate) fn has_pending(&self, control: &QueueControlState, queue: QueueId) -> bool {
        match self {
            Self::Semantic(io) => io.has_pending(control, queue),
            Self::Exact(_) => ExactFrontend::has_pending(control, queue),
        }
    }

    pub(crate) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut QueueControlState,
        queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        match self {
            Self::Semantic(io) => io.publish_completion(memory, control, queue, record),
            Self::Exact(_) => ExactFrontend::publish_completion(memory, control, queue, record),
        }
    }

    pub(crate) const fn stage_execution_fault(
        &mut self,
        event: EventRecord,
    ) -> ModelResult<ModelError> {
        match self {
            Self::Semantic(io) => Ok(io.stage_execution_fault(event)),
            Self::Exact(_) => ExactFrontend::stage_execution_fault(event),
        }
    }

    pub(crate) fn publish_fault(
        &mut self,
        memory: &PdmaMemoryMappings,
        control: &mut QueueControlState,
        trace: &mut TraceSink,
    ) -> PublicationOutcome {
        match self {
            Self::Semantic(io) => io.publish_fault(memory, control, trace),
            Self::Exact(io) => io.publish_fault(memory, control, trace),
        }
    }

    pub(crate) fn completion_blocked(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        match self {
            Self::Semantic(io) => io.completion_blocked(control, queue, publication_slots),
            Self::Exact(_) => ExactFrontend::completion_blocked(control, queue, publication_slots),
        }
    }

    pub(crate) fn fault_blocked(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        match self {
            Self::Semantic(io) => io.fault_blocked(control, queue, publication_slots),
            Self::Exact(_) => ExactFrontend::fault_blocked(control, queue, publication_slots),
        }
    }

    pub(crate) fn records_pending(
        &self,
        control: &QueueControlState,
        queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool> {
        match self {
            Self::Semantic(io) => Ok(io.records_pending(control, queue, completion)),
            Self::Exact(_) => ExactFrontend::records_pending(control, queue, completion),
        }
    }
}
