use super::PdmaNotificationEnvelope;
use hardware_interface_runtime::{InterruptLifecycleEvent, InterruptLifecycleObserver};
use pdma_model_contract::semantic::{TraceEvent, TraceEventKind};
use std::sync::Mutex;

/// Clone-shareable synchronous trace adapter for complete composite IRQ turns.
#[derive(Debug, Default)]
pub struct PdmaInterruptTrace {
    events: Mutex<Vec<TraceEvent>>,
}

impl PdmaInterruptTrace {
    /// Returns a stable snapshot of all lifecycle observations.
    pub fn events(&self) -> Vec<TraceEvent> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl<N: PdmaNotificationEnvelope> InterruptLifecycleObserver<N> for PdmaInterruptTrace {
    fn observe(&self, event: InterruptLifecycleEvent<'_, N>) {
        let (kind, detail) = match event {
            InterruptLifecycleEvent::Started { notification, .. } => (
                TraceEventKind::IrqHandlerStarted,
                match notification.notification() {
                    pdma_model_contract::semantic::IrqNotification::Line(_) => "line",
                    pdma_model_contract::semantic::IrqNotification::Message(_) => "message",
                },
            ),
            InterruptLifecycleEvent::Succeeded { .. } => {
                (TraceEventKind::IrqHandlerCompleted, "Success")
            }
            InterruptLifecycleEvent::Failed { .. } => {
                (TraceEventKind::IrqHandlerCompleted, "Error")
            }
        };
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sequence = events.len();
        events.push(TraceEvent::new(sequence, kind, None, None, None, detail));
    }
}
