use pdma_model_contract::{IrqNotification, TraceEvent, TraceEventKind};

#[derive(Debug, Default)]
pub(super) struct HostIrqTrace {
    events: Vec<TraceEvent>,
    next_sequence: usize,
}

impl HostIrqTrace {
    pub(super) fn handler_started(&mut self, notification: IrqNotification) {
        self.push(
            TraceEventKind::IrqHandlerStarted,
            notification_detail(notification),
        );
    }

    pub(super) fn handler_completed(&mut self, succeeded: bool) {
        self.push(
            TraceEventKind::IrqHandlerCompleted,
            if succeeded { "Success" } else { "Error" },
        );
    }

    pub(super) fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    fn push(&mut self, kind: TraceEventKind, detail: &'static str) {
        self.events.push(TraceEvent::new(
            self.next_sequence,
            kind,
            None,
            None,
            None,
            detail,
        ));
        self.next_sequence = self.next_sequence.saturating_add(1);
    }
}

const fn notification_detail(notification: IrqNotification) -> &'static str {
    match notification {
        IrqNotification::Line(_) => "line",
        IrqNotification::Message(_) => "message",
    }
}
