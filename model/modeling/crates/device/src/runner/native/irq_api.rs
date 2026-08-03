use super::{QueuePortDeviceModel, irq::IrqSourceObservation};
use crate::runner::bus::control::BusControlState;
use pdma_model_contract::{
    EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot, IrqSource, ModelError, ModelResult,
};

impl QueuePortDeviceModel {
    pub(in crate::runner) fn enable_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.enable(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        self.irq.configure_route(source, route)
    }

    pub(in crate::runner) fn disable_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.disable(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.mask(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn unmask_irq_source(
        &mut self,
        source: EpochIrqSource,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.unmask(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn ack_irq_source(
        &mut self,
        bus_control: &BusControlState,
        source: EpochIrqSource,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let records_pending = self.irq_records_pending(bus_control, source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.ack(source, records_pending);
        self.trace.irq_device_ack(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn latch_irq_source(&mut self, source: IrqSource) {
        let was_asserted = self.irq.line_asserted();
        if self.irq.latch(source) {
            self.trace.irq_source_latched(source);
        }
        self.trace_line_transition(was_asserted);
    }

    pub(in crate::runner) fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.irq.pending_snapshot()
    }

    pub(in crate::runner) fn take_irq_notification(&mut self) -> Option<IrqNotification> {
        let notification = self.irq.take_notification();
        if let Some(IrqNotification::Message(message)) = notification {
            self.trace.irq_message_emitted(message.source());
        }
        notification
    }

    pub(in crate::runner) fn observe_irq_source(
        &self,
        bus_control: &BusControlState,
        source: EpochIrqSource,
    ) -> ModelResult<(IrqSourceObservation, bool)> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let records_pending = self.irq_records_pending(bus_control, source)?;
        let observation = self.irq.observe(source);
        Ok((observation, records_pending))
    }

    pub(in crate::runner) fn irq_notification_ready(&self) -> bool {
        self.irq.notification_ready()
    }

    fn trace_line_transition(&mut self, was_asserted: bool) {
        match (was_asserted, self.irq.line_asserted()) {
            (false, true) => self.trace.irq_line_asserted(),
            (true, false) => self.trace.irq_line_deasserted(),
            (false, false) | (true, true) => {}
        }
    }

    fn validate_irq_source(&self, source: IrqSource) -> ModelResult<()> {
        match source {
            IrqSource::Completion(id) => self
                .completion(id)
                .map(|_| ())
                .map_err(|_| ModelError::UnknownIrqSource(source)),
            IrqSource::Event(id) => self
                .queue(id)
                .map(|_| ())
                .map_err(|_| ModelError::UnknownIrqSource(source)),
        }
    }

    fn irq_records_pending(
        &self,
        bus_control: &BusControlState,
        source: IrqSource,
    ) -> ModelResult<bool> {
        match source {
            IrqSource::Completion(id) => {
                let queue = self
                    .queues
                    .iter()
                    .find(|queue| queue.entry.completion_ring_id() == id)
                    .ok_or(ModelError::UnknownIrqSource(source))?;
                if queue.io.is_dma_bus() {
                    let active = bus_control
                        .queue(pdma_model_contract::abi::EpochQueue::new(
                            self.epoch,
                            queue.entry.queue_id(),
                        ))
                        .map_err(|_| ModelError::DmaSubmissionState)?;
                    Ok(active.completion.producer() != active.completion.consumer())
                } else {
                    self.completion(id).map(|ring| !ring.is_empty())
                }
            }
            IrqSource::Event(id) => {
                let queue = self.queue(id)?;
                if queue.io.is_dma_bus() {
                    let active = bus_control
                        .queue(pdma_model_contract::abi::EpochQueue::new(self.epoch, id))
                        .map_err(|_| ModelError::DmaSubmissionState)?;
                    Ok(active.event.producer() != active.event.consumer())
                } else {
                    Ok(self.events.iter().any(|event| event.queue_id() == id))
                }
            }
        }
    }
}
