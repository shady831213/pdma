use super::{FrontendBehavior, NormalizedFetch, PublicationOutcome};
use crate::ModelResult;
use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use crate::engine::ring::NativeRing;
use pdma_model_contract::semantic::{
    Command, CompletionRecord, EventRecord, QueueDirectoryEntry, QueueId,
};
use std::collections::VecDeque;

#[derive(Debug)]
pub(in crate::engine) struct SemanticFrontend {
    commands: NativeRing<Command>,
    completions: NativeRing<CompletionRecord>,
    events: VecDeque<EventRecord>,
    pending_fault: Option<(EventRecord, crate::ModelError)>,
}

impl SemanticFrontend {
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
}

impl FrontendBehavior for SemanticFrontend {
    fn fetch(
        &mut self,
        _memory: &PdmaMemoryMappings,
        _control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        Ok(self
            .commands
            .front()
            .cloned()
            .map(|command| NormalizedFetch::Command {
                command,
                completion_ring: entry.completion_ring_id(),
            }))
    }

    fn probe(
        &self,
        _memory: &PdmaMemoryMappings,
        _control: &QueueControlState,
        entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        Ok(self
            .commands
            .front()
            .cloned()
            .map(|command| NormalizedFetch::Command {
                command,
                completion_ring: entry.completion_ring_id(),
            }))
    }

    fn consume(&mut self, _control: &mut QueueControlState, _queue: QueueId) -> ModelResult<()> {
        let _consumed = self.commands.pop();
        Ok(())
    }

    fn has_pending(&self, _control: &QueueControlState, _queue: QueueId) -> bool {
        !self.commands.is_empty()
    }

    fn publish_completion(
        &mut self,
        _memory: &PdmaMemoryMappings,
        _control: &mut QueueControlState,
        _queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        if self.completions.push(record).is_ok() {
            PublicationOutcome::Published
        } else {
            PublicationOutcome::Retry
        }
    }

    fn stage_execution_fault(&mut self, event: EventRecord) -> ModelResult<crate::ModelError> {
        let source = crate::ModelError::from_memory_fault(event.buffer(), event.error_kind());
        self.pending_fault = Some((event, source));
        Ok(source)
    }

    fn publish_fault(
        &mut self,
        _memory: &PdmaMemoryMappings,
        _control: &mut QueueControlState,
        trace: &mut crate::engine::trace::TraceSink,
    ) -> PublicationOutcome {
        let Some((event, _)) = self.pending_fault.take() else {
            return PublicationOutcome::Retry;
        };
        self.events.push_back(event);
        trace.event_published(event);
        PublicationOutcome::Published
    }

    fn completion_blocked(
        &self,
        _control: &QueueControlState,
        _queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        self.completions.is_full() || self.completions.len() >= publication_slots
    }

    fn fault_blocked(
        &self,
        _control: &QueueControlState,
        _queue: QueueId,
        compat_publication_slots: usize,
    ) -> bool {
        self.events.len() >= compat_publication_slots
    }

    fn records_pending(
        &self,
        _control: &QueueControlState,
        _queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool> {
        if completion {
            Ok(!self.completions.is_empty())
        } else {
            Ok(!self.events.is_empty())
        }
    }
}
