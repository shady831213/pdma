use super::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{EpochIrqSource, IrqNotification, IrqRoute, IrqSource};

impl PdmaIrqController {
    pub(crate) fn enable_source(
        &mut self,
        source: EpochIrqSource,
        queues: &QueueBlock,
        trace: &mut TraceSink,
    ) -> ModelResult<()> {
        let source = self.validate_source(source, queues)?;
        let was_asserted = self.line_asserted();
        self.enable(source);
        self.trace_line_transition(was_asserted, trace);
        Ok(())
    }

    pub(crate) fn configure_source_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
        queues: &QueueBlock,
    ) -> ModelResult<()> {
        let source = self.validate_source(source, queues)?;
        self.configure_route(source, route)
    }

    pub(crate) fn disable_source(
        &mut self,
        source: EpochIrqSource,
        queues: &QueueBlock,
        trace: &mut TraceSink,
    ) -> ModelResult<()> {
        let source = self.validate_source(source, queues)?;
        let was_asserted = self.line_asserted();
        self.disable(source);
        self.trace_line_transition(was_asserted, trace);
        Ok(())
    }

    pub(crate) fn mask_source(
        &mut self,
        source: EpochIrqSource,
        queues: &QueueBlock,
        trace: &mut TraceSink,
    ) -> ModelResult<()> {
        let source = self.validate_source(source, queues)?;
        let was_asserted = self.line_asserted();
        self.mask(source);
        self.trace_line_transition(was_asserted, trace);
        Ok(())
    }

    pub(crate) fn unmask_source(
        &mut self,
        source: EpochIrqSource,
        queues: &QueueBlock,
        trace: &mut TraceSink,
    ) -> ModelResult<()> {
        let source = self.validate_source(source, queues)?;
        let was_asserted = self.line_asserted();
        self.unmask(source);
        self.trace_line_transition(was_asserted, trace);
        Ok(())
    }

    pub(crate) fn ack_source(
        &mut self,
        source: EpochIrqSource,
        queues: &QueueBlock,
        trace: &mut TraceSink,
    ) -> ModelResult<()> {
        let source = self.validate_source(source, queues)?;
        let records_pending = Self::records_pending(source, queues)?;
        let was_asserted = self.line_asserted();
        self.ack(source, records_pending);
        trace.irq_device_ack(source);
        self.trace_line_transition(was_asserted, trace);
        Ok(())
    }

    pub(crate) fn take_traced_notification(
        &mut self,
        trace: &mut TraceSink,
    ) -> Option<IrqNotification> {
        let notification = self.take_notification();
        if let Some(IrqNotification::Message(message)) = notification {
            trace.irq_message_emitted(message.source());
        }
        notification
    }

    pub(crate) fn observe_source(
        &self,
        source: EpochIrqSource,
        queues: &QueueBlock,
    ) -> ModelResult<(crate::irq::IrqSourceObservation, bool)> {
        let source = self.validate_source(source, queues)?;
        let records_pending = Self::records_pending(source, queues)?;
        Ok((self.observe(source), records_pending))
    }

    fn trace_line_transition(&self, was_asserted: bool, trace: &mut TraceSink) {
        match (was_asserted, self.line_asserted()) {
            (false, true) => trace.irq_line_asserted(),
            (true, false) => trace.irq_line_deasserted(),
            (false, false) | (true, true) => {}
        }
    }

    fn validate_source(
        &self,
        source: EpochIrqSource,
        queues: &QueueBlock,
    ) -> ModelResult<IrqSource> {
        if source.epoch() != self.epoch {
            return Err(ModelError::StaleDeviceEpoch {
                expected: self.epoch,
                actual: source.epoch(),
            });
        }
        let source = source.source();
        let valid = match source {
            IrqSource::Completion(completion) => queues.contains_completion(completion),
            IrqSource::Event(queue) => queues.entry(queue).is_ok(),
        };
        if valid {
            Ok(source)
        } else {
            Err(ModelError::UnknownIrqSource(source))
        }
    }

    fn records_pending(source: IrqSource, queues: &QueueBlock) -> ModelResult<bool> {
        match source {
            IrqSource::Completion(completion) => queues
                .completion_records_pending(completion)
                .map_err(|_| ModelError::UnknownIrqSource(source)),
            IrqSource::Event(queue) => queues
                .records_pending(queue, false)
                .map_err(|_| ModelError::UnknownIrqSource(source)),
        }
    }
}
