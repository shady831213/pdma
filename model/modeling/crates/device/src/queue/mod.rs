mod block;
pub(crate) mod exact;
pub(crate) mod io;
mod ring;
pub(crate) mod runnable;
mod semantic;

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub(crate) struct QueueBlock {
    contexts: Vec<PdmaQueue>,
}

use self::io::{NormalizedFetch, PublicationOutcome};
use self::semantic::SemanticQueueIo;
use crate::memory::PdmaMemoryMappings;
use crate::queue::exact::ExactQueueIo;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    Command, CommandId, CompletionRecord, CompletionRingId, EventRecord, QueueConfig,
    QueueDirectoryEntry, QueueId, TraceOperation,
};

#[derive(Debug)]
struct PdmaQueue {
    entry: QueueDirectoryEntry,
    io: QueueIo,
}

impl PdmaQueue {
    fn semantic(
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

    const fn exact(entry: QueueDirectoryEntry, io: ExactQueueIo) -> Self {
        Self {
            entry,
            io: QueueIo::Exact(io),
        }
    }
}

#[derive(Debug)]
enum QueueIo {
    Semantic(SemanticQueueIo),
    Exact(ExactQueueIo),
}

impl QueueIo {
    const fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    const fn exact(&self) -> Option<&ExactQueueIo> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(io) => Some(io),
        }
    }

    const fn exact_mut(&mut self) -> Option<&mut ExactQueueIo> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(io) => Some(io),
        }
    }

    fn submit_semantic(
        &mut self,
        queue: QueueId,
        command: Command,
    ) -> ModelResult<(CommandId, TraceOperation)> {
        match self {
            Self::Semantic(io) => io.submit(command),
            Self::Exact(_) => Err(ModelError::UnknownQueue(queue)),
        }
    }

    fn poll_semantic_completion(&mut self) -> Option<CompletionRecord> {
        match self {
            Self::Semantic(io) => io.poll_completion(),
            Self::Exact(_) => None,
        }
    }

    fn poll_semantic_event(&mut self, queue: QueueId) -> Option<EventRecord> {
        match self {
            Self::Semantic(io) => io.poll_event(queue),
            Self::Exact(_) => None,
        }
    }

    const fn pending_exact_fault(&self) -> Option<exact::PendingDmaResult> {
        match self {
            Self::Semantic(_) => None,
            Self::Exact(io) => io.pending_result(),
        }
    }

    fn probe(
        &self,
        memory: &PdmaMemoryMappings,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(io) => Ok(io.probe(entry)),
            Self::Exact(io) => io.probe(memory, entry),
        }
    }

    fn fetch(
        &mut self,
        memory: &PdmaMemoryMappings,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        match self {
            Self::Semantic(io) => Ok(io.fetch(entry)),
            Self::Exact(io) => io.fetch(memory, entry),
        }
    }

    fn consume(&mut self) -> ModelResult<()> {
        match self {
            Self::Semantic(io) => {
                io.consume();
                Ok(())
            }
            Self::Exact(io) => io.consume(),
        }
    }

    fn has_pending(&self) -> bool {
        match self {
            Self::Semantic(io) => io.has_pending(),
            Self::Exact(io) => io.has_pending(),
        }
    }

    #[cfg(test)]
    const fn semantic_cursors(&self) -> Option<((usize, usize), (usize, usize))> {
        match self {
            Self::Semantic(io) => Some(io.cursors()),
            Self::Exact(_) => None,
        }
    }

    fn publish_completion(
        &mut self,
        memory: &PdmaMemoryMappings,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        match self {
            Self::Semantic(io) => io.publish_completion(record),
            Self::Exact(io) => io.publish_completion(memory, record),
        }
    }

    const fn stage_execution_fault(&mut self, event: EventRecord) -> ModelResult<ModelError> {
        match self {
            Self::Semantic(io) => Ok(io.stage_execution_fault(event)),
            Self::Exact(_) => ExactQueueIo::stage_execution_fault(event),
        }
    }

    fn publish_fault(&mut self, memory: &PdmaMemoryMappings) -> io::FaultPublication {
        match self {
            Self::Semantic(io) => io.publish_fault(),
            Self::Exact(io) => io.publish_fault(memory),
        }
    }

    fn completion_blocked(&self, publication_slots: usize) -> bool {
        match self {
            Self::Semantic(io) => io.completion_blocked(publication_slots),
            Self::Exact(io) => io.completion_blocked(publication_slots),
        }
    }

    fn fault_blocked(&self, publication_slots: usize) -> bool {
        match self {
            Self::Semantic(io) => io.fault_blocked(publication_slots),
            Self::Exact(io) => io.fault_blocked(publication_slots),
        }
    }

    fn records_pending(&self, completion: bool) -> ModelResult<bool> {
        match self {
            Self::Semantic(io) => Ok(io.records_pending(completion)),
            Self::Exact(io) => io.records_pending(completion),
        }
    }
}
