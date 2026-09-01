use super::DeviceCore;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
};

impl DeviceCore {
    pub(crate) fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.enable(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(crate) fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        self.irq.configure_route(source, route)
    }

    pub(crate) fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.disable(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(crate) fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.mask(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(crate) fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.unmask(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(crate) fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let records_pending = self.irq_records_pending(source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.ack(source, records_pending);
        self.trace.irq_device_ack(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(crate) fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.irq.pending_snapshot()
    }

    pub(crate) fn take_irq_notification(&mut self) -> Option<IrqNotification> {
        let notification = self.irq.take_notification();
        if let Some(IrqNotification::Message(message)) = notification {
            self.trace.irq_message_emitted(message.source());
        }
        notification
    }

    pub(crate) fn observe_irq_source(
        &self,
        source: EpochIrqSource,
    ) -> ModelResult<(crate::irq::IrqSourceObservation, bool)> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let records_pending = self.irq_records_pending(source)?;
        Ok((self.irq.observe(source), records_pending))
    }

    pub(crate) fn irq_notification_ready(&self) -> bool {
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
        let valid = match source {
            IrqSource::Completion(completion) => self.queues.contains_completion(completion),
            IrqSource::Event(queue) => self.queues.entry(queue).is_ok(),
        };
        if valid {
            Ok(())
        } else {
            Err(ModelError::UnknownIrqSource(source))
        }
    }

    fn irq_records_pending(&self, source: IrqSource) -> ModelResult<bool> {
        match source {
            IrqSource::Completion(completion) => self
                .queues
                .completion_records_pending(completion)
                .map_err(|_| ModelError::UnknownIrqSource(source)),
            IrqSource::Event(queue) => self
                .queues
                .records_pending(queue, false)
                .map_err(|_| ModelError::UnknownIrqSource(source)),
        }
    }
}
