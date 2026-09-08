use super::io::{NormalizedFetch, PublicationOutcome};
use crate::ModelResult;
use crate::queue::ring::NativeRing;
use pdma_model_contract::semantic::{
    Command, CompletionRecord, EventRecord, QueueDirectoryEntry, QueueId,
};
use std::collections::VecDeque;

#[derive(Debug)]
pub(super) struct SemanticQueueIo {
    commands: NativeRing<Command>,
    completions: NativeRing<CompletionRecord>,
    events: VecDeque<EventRecord>,
    pending_fault: Option<(EventRecord, crate::ModelError)>,
}

impl SemanticQueueIo {
    pub(super) fn new(command_depth: usize, completion_depth: usize) -> ModelResult<Self> {
        Ok(Self {
            commands: NativeRing::with_capacity(command_depth)?,
            completions: NativeRing::with_capacity(completion_depth)?,
            events: VecDeque::new(),
            pending_fault: None,
        })
    }

    pub(super) fn submit(
        &mut self,
        command: Command,
    ) -> ModelResult<(
        pdma_model_contract::semantic::CommandId,
        pdma_model_contract::semantic::TraceOperation,
    )> {
        let identity = (command.command_id(), command.operation().trace_operation());
        self.commands.push(command).map(|()| identity)
    }

    pub(super) fn poll_completion(&mut self) -> Option<CompletionRecord> {
        self.completions.pop()
    }

    pub(super) fn poll_event(&mut self, queue: QueueId) -> Option<EventRecord> {
        let index = self
            .events
            .iter()
            .position(|event| event.queue_id() == queue)?;
        self.events.remove(index)
    }
    pub(super) fn fetch(&self, entry: QueueDirectoryEntry) -> Option<NormalizedFetch> {
        self.commands
            .front()
            .cloned()
            .map(|command| NormalizedFetch::Command {
                command,
                completion_ring: entry.completion_ring_id(),
            })
    }

    pub(super) fn probe(&self, entry: QueueDirectoryEntry) -> Option<NormalizedFetch> {
        self.commands
            .front()
            .cloned()
            .map(|command| NormalizedFetch::Command {
                command,
                completion_ring: entry.completion_ring_id(),
            })
    }

    pub(super) fn consume(&mut self) {
        let _consumed = self.commands.pop();
    }

    pub(super) const fn has_pending(&self) -> bool {
        !self.commands.is_empty()
    }

    #[cfg(test)]
    pub(super) const fn cursors(&self) -> ((usize, usize), (usize, usize)) {
        (self.commands.cursors(), self.completions.cursors())
    }

    pub(super) fn publish_completion(&mut self, record: CompletionRecord) -> PublicationOutcome {
        if self.completions.push(record).is_ok() {
            PublicationOutcome::Published
        } else {
            PublicationOutcome::Retry
        }
    }

    pub(super) const fn stage_execution_fault(&mut self, event: EventRecord) -> crate::ModelError {
        let source = crate::ModelError::from_memory_fault(event.buffer(), event.error_kind());
        self.pending_fault = Some((event, source));
        source
    }

    pub(super) fn publish_fault(&mut self) -> super::io::FaultPublication {
        let Some((event, _)) = self.pending_fault.take() else {
            return super::io::FaultPublication::Retry;
        };
        self.events.push_back(event);
        super::io::FaultPublication::Semantic(event)
    }

    pub(super) fn completion_blocked(&self, publication_slots: usize) -> bool {
        self.completions.is_full() || self.completions.len() >= publication_slots
    }

    pub(super) fn fault_blocked(&self, compat_publication_slots: usize) -> bool {
        self.events.len() >= compat_publication_slots
    }

    pub(super) fn records_pending(&self, completion: bool) -> bool {
        if completion {
            !self.completions.is_empty()
        } else {
            !self.events.is_empty()
        }
    }
}
