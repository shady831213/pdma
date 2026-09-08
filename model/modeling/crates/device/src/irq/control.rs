use super::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{IrqNotification, IrqSource};

impl PdmaIrqController {
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

    pub(super) fn trace_line_transition(&self, was_asserted: bool, trace: &mut TraceSink) {
        match (was_asserted, self.line_asserted()) {
            (false, true) => trace.irq_line_asserted(),
            (true, false) => trace.irq_line_deasserted(),
            (false, false) | (true, true) => {}
        }
    }

    pub(super) fn records_pending(source: IrqSource, queues: &QueueBlock) -> ModelResult<bool> {
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
