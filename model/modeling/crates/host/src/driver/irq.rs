//! Driver-side PDMA IRQ source demultiplexing.

use super::AsyncDriver;
use crate::AsyncDevicePort;
use pdma_model_contract::{
    CompletionRecord, EventRecord, IrqNotification, IrqSource, ModelError, ModelResult,
};

/// One record drained by the PDMA software handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverIrqRecord {
    /// Successful command completion.
    Completion(CompletionRecord),
    /// Fault or asynchronous queue event.
    Event(EventRecord),
}

impl AsyncDriver {
    /// Demultiplexes one IRQ, drains its ring, then clears the device-local cause.
    pub async fn service_irq_on(
        &mut self,
        device: &mut impl AsyncDevicePort,
        notification: IrqNotification,
    ) -> ModelResult<Vec<DriverIrqRecord>> {
        let source = match notification {
            IrqNotification::Line(snapshot) => snapshot.cause().source(),
            IrqNotification::Message(message) => message.source(),
        };
        let records = match source {
            IrqSource::Completion(completion_ring_id)
                if completion_ring_id == self.completion_ring_id() =>
            {
                self.drain_completions(device).await?
            }
            IrqSource::Event(queue_id) if queue_id == self.queue_id() => {
                self.drain_events(device).await?
            }
            _ => return Err(ModelError::UnknownIrqSource(source)),
        };
        device.ack_irq_source(source).await?;
        Ok(records)
    }

    async fn drain_completions(
        &mut self,
        device: &mut impl AsyncDevicePort,
    ) -> ModelResult<Vec<DriverIrqRecord>> {
        let mut records = Vec::new();
        while let Some(completion) = self.poll_completion_on(device).await? {
            records.push(DriverIrqRecord::Completion(completion));
        }
        Ok(records)
    }

    async fn drain_events(
        &mut self,
        device: &mut impl AsyncDevicePort,
    ) -> ModelResult<Vec<DriverIrqRecord>> {
        let mut records = Vec::new();
        while let Some(event) = self.poll_event_on(device).await? {
            records.push(DriverIrqRecord::Event(event));
        }
        Ok(records)
    }
}
