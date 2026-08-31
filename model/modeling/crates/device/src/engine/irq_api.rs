//! IRQ control methods on the shared execution engine.

use super::DeviceExecution;
use crate::abi::ring::control::QueueControlState;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct IrqSourceObservation {
    pub(crate) route: IrqRoute,
    pub(crate) pending_messages: usize,
    state: u8,
}

impl IrqSourceObservation {
    pub(crate) const fn new(route: IrqRoute, pending_messages: usize, state: u8) -> Self {
        Self {
            route,
            pending_messages,
            state,
        }
    }

    pub(crate) const fn enabled(self) -> bool {
        self.state & 1 != 0
    }

    pub(crate) const fn masked(self) -> bool {
        self.state & 2 != 0
    }

    pub(crate) const fn cause_pending(self) -> bool {
        self.state & 4 != 0
    }

    pub(crate) const fn deliverable(self) -> bool {
        self.state & 8 != 0
    }

    pub(crate) const fn line_asserted(self) -> bool {
        self.state & 16 != 0
    }
}

impl DeviceExecution {
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

    pub(crate) fn ack_irq_source(
        &mut self,
        queue_control: &QueueControlState,
        source: EpochIrqSource,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let records_pending = self.irq_records_pending(queue_control, source)?;
        let was_asserted = self.irq.line_asserted();
        self.irq.ack(source, records_pending);
        self.trace.irq_device_ack(source);
        self.trace_line_transition(was_asserted);
        Ok(())
    }

    pub(crate) fn latch_irq_source(&mut self, source: IrqSource) {
        let was_asserted = self.irq.line_asserted();
        if self.irq.latch(source) {
            self.trace.irq_source_latched(source);
        }
        self.trace_line_transition(was_asserted);
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
        queue_control: &QueueControlState,
        source: EpochIrqSource,
    ) -> ModelResult<(IrqSourceObservation, bool)> {
        self.validate_epoch(source.epoch())?;
        let source = source.source();
        self.validate_irq_source(source)?;
        let records_pending = self.irq_records_pending(queue_control, source)?;
        let observation = self.irq.observe(source);
        Ok((observation, records_pending))
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
        match source {
            IrqSource::Completion(id) => self
                .queue_by_completion(id)
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
        queue_control: &QueueControlState,
        source: IrqSource,
    ) -> ModelResult<bool> {
        match source {
            IrqSource::Completion(id) => {
                let queue = self
                    .queues
                    .iter()
                    .find(|queue| queue.entry.completion_ring_id() == id)
                    .ok_or(ModelError::UnknownIrqSource(source))?;
                queue
                    .frontend
                    .records_pending(queue_control, queue.entry.queue_id(), true)
            }
            IrqSource::Event(id) => {
                let queue = self.queue(id)?;
                queue.frontend.records_pending(queue_control, id, false)
            }
        }
    }
}
