use super::QueuePortDeviceModel;
use pdma_model_contract::{
    IrqNotification, IrqRoute, IrqSnapshot, IrqSource, ModelError, ModelResult,
};

impl QueuePortDeviceModel {
    pub(in crate::runner) fn enable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.enable(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn configure_irq_route(
        &mut self,
        source: IrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.validate_irq_source(source)?;
        self.irq.configure_route(source, route)
    }

    pub(in crate::runner) fn disable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.disable(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn mask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.mask(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn unmask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.unmask(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(in crate::runner) fn ack_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        self.validate_irq_source(source)?;
        let records_pending = self.irq_records_pending(source)?;
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

    fn irq_records_pending(&self, source: IrqSource) -> ModelResult<bool> {
        match source {
            IrqSource::Completion(id) => self.completion(id).map(|ring| !ring.is_empty()),
            IrqSource::Event(id) => {
                self.queue(id)?;
                Ok(self.events.iter().any(|event| event.queue_id() == id))
            }
        }
    }
}
