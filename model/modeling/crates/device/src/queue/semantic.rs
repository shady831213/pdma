use super::io::{NormalizedFetch, PublicationOutcome};
use crate::ModelResult;
use crate::queue::ring::NativeRing;
use pdma_model_contract::semantic::{
    Command, CompletionRecord, EventRecord, QueueDirectoryEntry, QueueId,
};
use std::collections::VecDeque;

#[derive(Debug)]
pub(crate) struct SemanticQueueIo {
    commands: NativeRing<Command>,
    completions: NativeRing<CompletionRecord>,
    events: VecDeque<EventRecord>,
    pending_fault: Option<(EventRecord, crate::ModelError)>,
}

impl SemanticQueueIo {
    pub(crate) fn new(command_depth: usize, completion_depth: usize) -> ModelResult<Self> {
        Ok(Self {
            commands: NativeRing::with_capacity(command_depth)?,
            completions: NativeRing::with_capacity(completion_depth)?,
            events: VecDeque::new(),
            pending_fault: None,
        })
    }

    pub(crate) fn submit(
        &mut self,
        command: Command,
    ) -> ModelResult<(
        pdma_model_contract::semantic::CommandId,
        pdma_model_contract::semantic::TraceOperation,
    )> {
        let identity = (command.command_id(), command.operation().trace_operation());
        self.commands.push(command).map(|()| identity)
    }

    pub(crate) fn poll_completion(&mut self) -> Option<CompletionRecord> {
        self.completions.pop()
    }

    pub(crate) fn poll_event(&mut self, queue: QueueId) -> Option<EventRecord> {
        let index = self
            .events
            .iter()
            .position(|event| event.queue_id() == queue)?;
        self.events.remove(index)
    }
    pub(crate) fn fetch(&self, entry: QueueDirectoryEntry) -> Option<NormalizedFetch> {
        self.commands
            .front()
            .cloned()
            .map(|command| NormalizedFetch::Command {
                command,
                completion_ring: entry.completion_ring_id(),
            })
    }

    pub(crate) fn probe(&self, entry: QueueDirectoryEntry) -> Option<NormalizedFetch> {
        self.commands
            .front()
            .cloned()
            .map(|command| NormalizedFetch::Command {
                command,
                completion_ring: entry.completion_ring_id(),
            })
    }

    pub(crate) fn consume(&mut self) {
        let _consumed = self.commands.pop();
    }

    pub(crate) const fn has_pending(&self) -> bool {
        !self.commands.is_empty()
    }

    pub(crate) fn publish_completion(&mut self, record: CompletionRecord) -> PublicationOutcome {
        if self.completions.push(record).is_ok() {
            PublicationOutcome::Published
        } else {
            PublicationOutcome::Retry
        }
    }

    pub(crate) const fn stage_execution_fault(&mut self, event: EventRecord) -> crate::ModelError {
        let source = crate::ModelError::from_memory_fault(event.buffer(), event.error_kind());
        self.pending_fault = Some((event, source));
        source
    }

    pub(crate) fn publish_fault(&mut self) -> super::io::FaultPublication {
        let Some((event, _)) = self.pending_fault.take() else {
            return super::io::FaultPublication::Retry;
        };
        self.events.push_back(event);
        super::io::FaultPublication::Semantic(event)
    }

    pub(crate) fn completion_blocked(&self, publication_slots: usize) -> bool {
        self.completions.is_full() || self.completions.len() >= publication_slots
    }

    pub(crate) fn fault_blocked(&self, compat_publication_slots: usize) -> bool {
        self.events.len() >= compat_publication_slots
    }

    pub(crate) fn records_pending(&self, completion: bool) -> bool {
        if completion {
            !self.completions.is_empty()
        } else {
            !self.events.is_empty()
        }
    }
}
