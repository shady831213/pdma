//! Driver-side PDMA IRQ source demultiplexing.

use crate::ModelError;
use crate::ModelResult;

use super::AsyncDriver;
use crate::DriverPort;
use pdma_model_contract::semantic::{CompletionRecord, EventRecord, IrqNotification, IrqSource};

/// One record drained by the PDMA software handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverIrqRecord {
    /// Successful command completion.
    Completion(CompletionRecord),
    /// Fault or asynchronous queue event.
    Event(EventRecord),
}

impl AsyncDriver {
    /// Demultiplexes one IRQ, drains its ring, then acknowledges the device cause.
    pub async fn service_irq_on(
        &mut self,
        device: &mut impl DriverPort,
        notification: IrqNotification,
    ) -> ModelResult<Vec<DriverIrqRecord>> {
        let source = self.validate_irq_notification(notification)?;
        let records = self.drain_irq_source_on(device, source).await?;
        device.ack_irq_source(source).await?;
        Ok(records)
    }

    pub(crate) fn validate_irq_notification(
        &self,
        notification: IrqNotification,
    ) -> ModelResult<pdma_model_contract::semantic::EpochIrqSource> {
        self.core.accounting.validate()?;
        let source = notification.source_capability();
        let expected_epoch = self.core.queue_config.epoch();
        if source.epoch() != expected_epoch {
            return Err(ModelError::StaleDeviceEpoch {
                expected: expected_epoch,
                actual: source.epoch(),
            });
        }
        match source.source() {
            IrqSource::Completion(completion_ring_id)
                if completion_ring_id == self.completion_ring_id() =>
            {
                Ok(source)
            }
            IrqSource::Event(queue_id) if queue_id == self.queue_id() => Ok(source),
            _ => Err(ModelError::UnknownIrqSource(source.source())),
        }
    }

    pub(crate) async fn drain_irq_source_on(
        &mut self,
        device: &mut impl DriverPort,
        source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> ModelResult<Vec<DriverIrqRecord>> {
        match source.source() {
            IrqSource::Completion(_) => self.drain_completions(device).await,
            IrqSource::Event(_) => self.drain_events(device).await,
        }
    }

    async fn drain_completions(
        &mut self,
        device: &mut impl DriverPort,
    ) -> ModelResult<Vec<DriverIrqRecord>> {
        let mut records = Vec::new();
        while let Some(completion) = self.poll_completion_on(device).await? {
            records.push(DriverIrqRecord::Completion(completion));
        }
        Ok(records)
    }

    async fn drain_events(
        &mut self,
        device: &mut impl DriverPort,
    ) -> ModelResult<Vec<DriverIrqRecord>> {
        let mut records = Vec::new();
        while let Some(event) = self.poll_event_on(device).await? {
            records.push(DriverIrqRecord::Event(event));
        }
        Ok(records)
    }
}
