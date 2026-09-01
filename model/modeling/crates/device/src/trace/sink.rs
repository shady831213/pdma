mod irq;

use crate::trace::event;
use pdma_model_contract::semantic::completion::CompletionRecord;
use pdma_model_contract::semantic::event::EventRecord;
use pdma_model_contract::semantic::ids::{CommandId, QueueId};
use pdma_model_contract::semantic::trace::{TraceEvent, TraceEventKind, TraceOperation};

/// In-memory deterministic trace sink for async PDMA lifecycle events.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct TraceSink {
    enabled: bool,
    events: Vec<TraceEvent>,
    next_sequence: usize,
    command_operations: Vec<((QueueId, CommandId), TraceOperation)>,
}

impl TraceSink {
    /// Enables event capture.
    pub(crate) const fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables event capture without clearing existing events.
    pub(crate) const fn disable(&mut self) {
        self.enabled = false;
    }

    /// Removes captured events and resets deterministic sequence ordinals.
    pub(crate) fn clear(&mut self) {
        self.events.clear();
        self.command_operations.clear();
        self.next_sequence = 0;
    }

    /// Captured trace events in deterministic order.
    pub(crate) fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    /// Exports captured events as minimal Chrome/Perfetto JSON.
    pub(crate) fn to_json(&self) -> String {
        super::json::export(&self.events)
    }

    pub(crate) fn command_submitted(
        &mut self,
        queue_id: QueueId,
        operation: TraceOperation,
        command_id: CommandId,
    ) {
        if !self.enabled {
            return;
        }
        self.remember_operation(queue_id, command_id, operation);
        self.push_command(
            TraceEventKind::CommandSubmitted,
            operation,
            queue_id,
            command_id,
            "command submitted",
        );
    }

    pub(crate) fn doorbell_runnable(&mut self, queue_id: QueueId) {
        self.push(
            TraceEventKind::DoorbellRunnable,
            None,
            Some(queue_id),
            None,
            "runnable",
        );
    }

    pub(crate) fn scheduled(
        &mut self,
        queue_id: QueueId,
        operation: TraceOperation,
        command_id: CommandId,
    ) {
        self.push_command(
            TraceEventKind::Scheduled,
            operation,
            queue_id,
            command_id,
            "lowered",
        );
    }

    pub(crate) fn execution_succeeded(
        &mut self,
        queue_id: QueueId,
        operation: TraceOperation,
        command_id: CommandId,
    ) {
        self.push_command(
            TraceEventKind::ExecutionSucceeded,
            operation,
            queue_id,
            command_id,
            "Success",
        );
    }

    pub(crate) fn execution_faulted(&mut self, record: EventRecord) {
        self.push(
            TraceEventKind::ExecutionFaulted,
            self.operation_for(record.queue_id(), record.command_id()),
            Some(record.queue_id()),
            Some(record.command_id()),
            event::event_detail(record),
        );
    }

    pub(crate) fn completion_published(&mut self, record: CompletionRecord) {
        self.push(
            TraceEventKind::CompletionPublished,
            None,
            Some(record.queue_id()),
            Some(record.command_id()),
            event::completion_detail(record),
        );
    }

    pub(crate) fn completion_polled(&mut self, record: CompletionRecord) {
        self.push(
            TraceEventKind::CompletionPolled,
            None,
            Some(record.queue_id()),
            Some(record.command_id()),
            "completion poll Some",
        );
    }

    pub(crate) fn event_published(&mut self, record: EventRecord) {
        self.push(
            TraceEventKind::EventPublished,
            None,
            Some(record.queue_id()),
            Some(record.command_id()),
            event::event_detail(record),
        );
    }

    pub(crate) fn dma_event_published(&mut self, queue_id: QueueId, command_id: CommandId) {
        self.push(
            TraceEventKind::EventPublished,
            None,
            Some(queue_id),
            Some(command_id),
            "ABI event",
        );
    }

    pub(crate) fn event_poll(&mut self, queue_id: QueueId, record: Option<EventRecord>) {
        match record {
            Some(event) => self.push(
                TraceEventKind::EventPolled,
                None,
                Some(event.queue_id()),
                Some(event.command_id()),
                "event poll Some",
            ),
            None => self.push(
                TraceEventKind::EventPolled,
                None,
                Some(queue_id),
                None,
                "event poll None",
            ),
        }
    }

    pub(crate) fn stall(&mut self, queue_id: Option<QueueId>, detail: &'static str) {
        self.push(TraceEventKind::Stall, None, queue_id, None, detail);
    }

    pub(crate) fn idle(&mut self) {
        self.push(TraceEventKind::Idle, None, None, None, "idle");
    }

    fn remember_operation(
        &mut self,
        queue_id: QueueId,
        command_id: CommandId,
        operation: TraceOperation,
    ) {
        if self.operation_for(queue_id, command_id).is_none() {
            self.command_operations
                .push(((queue_id, command_id), operation));
        }
    }

    fn operation_for(&self, queue_id: QueueId, command_id: CommandId) -> Option<TraceOperation> {
        self.command_operations
            .iter()
            .find(|((known_queue, known_id), _operation)| {
                *known_queue == queue_id && *known_id == command_id
            })
            .map(|(_key, operation)| *operation)
    }

    fn push_command(
        &mut self,
        kind: TraceEventKind,
        operation: TraceOperation,
        queue_id: QueueId,
        command_id: CommandId,
        detail: &'static str,
    ) {
        self.push(
            kind,
            Some(operation),
            Some(queue_id),
            Some(command_id),
            detail,
        );
    }

    fn push(
        &mut self,
        kind: TraceEventKind,
        operation: Option<TraceOperation>,
        queue_id: Option<QueueId>,
        command_id: Option<CommandId>,
        detail: &'static str,
    ) {
        if self.enabled {
            self.events.push(TraceEvent::new(
                self.next_sequence,
                kind,
                operation,
                queue_id,
                command_id,
                detail,
            ));
            self.next_sequence = self.next_sequence.saturating_add(1);
        }
    }
}
