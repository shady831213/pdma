pub(super) mod command;

use super::super::execution::ExecutionCommand;
use super::{NormalizedFetch, PublicationOutcome, QueueFrontend};
use crate::ModelResult;
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::bus::control::BusControlState;
use crate::runner::native::ring::NativeRing;
use pdma_model_contract::semantic::desc::ExternalCommand;
use pdma_model_contract::semantic::{CompletionRecord, EventRecord, QueueDirectoryEntry, QueueId};
use std::collections::VecDeque;

#[derive(Debug)]
pub(super) struct SemanticFrontend {
    commands: NativeRing<ExecutionCommand>,
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
}

impl QueueFrontend for SemanticFrontend {
    fn submit_compat(
        &mut self,
        _queue: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<(
        pdma_model_contract::semantic::CommandId,
        pdma_model_contract::semantic::TraceOperation,
    )> {
        let command = command::normalize(command);
        let identity = (command.command_id(), command.trace_operation());
        self.commands.push(command).map(|()| identity)
    }

    fn fetch(
        &mut self,
        _memory: &PdmaMemoryMappings,
        _control: &BusControlState,
        _entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        Ok(self.commands.front().cloned().map(NormalizedFetch::Command))
    }

    fn probe(
        &self,
        _memory: &PdmaMemoryMappings,
        _control: &BusControlState,
        _entry: QueueDirectoryEntry,
    ) -> ModelResult<Option<NormalizedFetch>> {
        Ok(self.commands.front().cloned().map(NormalizedFetch::Command))
    }

    fn consume(&mut self, _control: &mut BusControlState, _queue: QueueId) -> ModelResult<()> {
        let _consumed = self.commands.pop();
        Ok(())
    }

    fn has_pending(&self, _control: &BusControlState, _queue: QueueId) -> bool {
        !self.commands.is_empty()
    }

    fn publish_completion(
        &mut self,
        _memory: &PdmaMemoryMappings,
        _control: &mut BusControlState,
        _queue: QueueId,
        record: CompletionRecord,
    ) -> PublicationOutcome {
        if self.completions.push(record).is_ok() {
            PublicationOutcome::Published
        } else {
            PublicationOutcome::Retry
        }
    }

    fn stage_execution_fault(
        &mut self,
        fault: super::super::retirement::FaultRetirement,
    ) -> ModelResult<crate::ModelError> {
        let source =
            crate::ModelError::from_memory_fault(fault.fault.buffer(), fault.fault.error_kind());
        self.pending_fault = Some((
            EventRecord::from_memory_fault(fault.queue_id, fault.command_id, fault.fault),
            source,
        ));
        Ok(source)
    }

    fn publish_fault(
        &mut self,
        _memory: &PdmaMemoryMappings,
        _control: &mut BusControlState,
        trace: &mut crate::trace::TraceSink,
    ) -> PublicationOutcome {
        let Some((event, _)) = self.pending_fault.take() else {
            return PublicationOutcome::Retry;
        };
        self.events.push_back(event);
        trace.event_published(event);
        PublicationOutcome::Published
    }

    fn pending_fault_bytes(&self) -> Option<[u8; 64]> {
        None
    }

    fn poll_compat_completion(&mut self) -> Option<CompletionRecord> {
        self.completions.pop()
    }

    fn poll_compat_event(&mut self, queue: QueueId) -> Option<EventRecord> {
        let index = self
            .events
            .iter()
            .position(|event| event.queue_id() == queue)?;
        self.events.remove(index)
    }

    fn completion_blocked(
        &self,
        _control: &BusControlState,
        _queue: QueueId,
        publication_slots: usize,
    ) -> bool {
        self.completions.is_full() || self.completions.len() >= publication_slots
    }

    fn fault_blocked(
        &self,
        _control: &BusControlState,
        _queue: QueueId,
        compat_publication_slots: usize,
    ) -> bool {
        self.events.len() >= compat_publication_slots
    }

    fn records_pending(
        &self,
        _control: &BusControlState,
        _queue: QueueId,
        completion: bool,
    ) -> ModelResult<bool> {
        if completion {
            Ok(!self.completions.is_empty())
        } else {
            Ok(!self.events.is_empty())
        }
    }

    fn accepts_exact_control(&self) -> bool {
        false
    }
}
