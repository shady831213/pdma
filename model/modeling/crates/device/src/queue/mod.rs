#[doc(hidden)]
pub(crate) mod block;
#[doc(hidden)]
pub(crate) mod exact;
#[doc(hidden)]
pub(crate) mod io;
pub(crate) mod issue;
#[doc(hidden)]
pub(crate) mod ring;
#[doc(hidden)]
pub(crate) mod semantic;
pub(crate) mod stage;

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub(crate) struct QueueBlock {
    pub(crate) contexts: Vec<PdmaQueue>,
}

use self::io::{NormalizedFetch, PublicationOutcome};
use self::semantic::SemanticQueueIo;
use crate::dma::PdmaMemoryMappings;
use crate::queue::exact::ExactQueueIo;
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
            io: QueueIo::Semantic(SemanticQueueIo::new(
                config.command_depth(),
                config.completion_depth(),
            )?),
        })
    }

    pub(crate) const fn exact(entry: QueueDirectoryEntry, io: ExactQueueIo) -> Self {
        Self {
            entry,
            io: QueueIo::Exact(io),
        }
    }
}

#[derive(Debug)]
pub(crate) enum QueueIo {
    Semantic(SemanticQueueIo),
    Exact(ExactQueueIo),
}

impl QueueIo {
    pub(crate) const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    pub(crate) const fn exact(&self) -> Option<&ExactQueueIo> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(io) => Some(io),
        }
    }

    pub(crate) const fn exact_mut(&mut self) -> Option<&mut ExactQueueIo> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(io) => Some(io),
        }
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

    pub(crate) const fn pending_exact_fault(&self) -> Option<exact::PendingDmaResult> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(io) => io.pending_result(),
        }
    }

    pub(crate) fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(io) => Ok(io.probe(entry)),
            Self::Exact(io) => io.probe(memory, entry),
        }
    }

    pub(crate) fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(io) => Ok(io.fetch(entry)),
            Self::Exact(io) => io.fetch(memory, entry),
        }
    }

    pub(crate) fn consume(&mut self) -> ModelResult<()> {
        match self {
            Self::Semantic(io) => {
                io.consume();
                Ok(())
            }
            Self::Exact(io) => io.consume(),
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        match self {
            Self::Semantic(io) => io.has_pending(),
            Self::Exact(io) => io.has_pending(),
        }
    }

    #[cfg(test)]
    pub(crate) const fn semantic_cursors(&self) -> Option<((usize, usize), (usize, usize))> {
        match self {
            Self::Semantic(io) => Some(io.cursors()),
            Self::Exact(_) => None,
        }
    }

    pub(crate) fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        match self {
            Self::Semantic(io) => io.publish_completion(record),
            Self::Exact(io) => io.publish_completion(memory, record),
        }
    }

    pub(crate) const fn stage_execution_fault(
        &mut self,
        event: EventRecord,
    ) -> ModelResult<ModelError> {
        match self {
            Self::Semantic(io) => Ok(io.stage_execution_fault(event)),
            Self::Exact(_) => ExactQueueIo::stage_execution_fault(event),
        }
    }

    pub(crate) fn publish_fault(&mut self, memory: &PdmaMemoryMappings) -> io::FaultPublication {
        match self {
            Self::Semantic(io) => io.publish_fault(),
            Self::Exact(io) => io.publish_fault(memory),
        }
    }

    pub(crate) fn completion_blocked(&self, publication_slots: usize) -> bool {
        match self {
            Self::Semantic(io) => io.completion_blocked(publication_slots),
            Self::Exact(io) => io.completion_blocked(publication_slots),
        }
    }

    pub(crate) fn fault_blocked(&self, publication_slots: usize) -> bool {
        match self {
            Self::Semantic(io) => io.fault_blocked(publication_slots),
            Self::Exact(io) => io.fault_blocked(publication_slots),
        }
    }

    pub(crate) fn records_pending(&self, completion: bool) -> ModelResult<bool> {
        match self {
            Self::Semantic(io) => Ok(io.records_pending(completion)),
            Self::Exact(io) => io.records_pending(completion),
        }
    }
}
