use super::TraceSink;
use pdma_model_contract::semantic::ids::QueueId;
use pdma_model_contract::semantic::irq::IrqSource;
use pdma_model_contract::semantic::trace::TraceEventKind;

impl TraceSink {
    pub(crate) fn irq_source_latched(&mut self, source: IrqSource) {
        self.push(
            TraceEventKind::IrqSourceLatched,
            None,
            irq_queue(source),
            None,
            irq_detail(source),
        );
    }

    pub(crate) fn irq_line_asserted(&mut self) {
        self.push(
            TraceEventKind::IrqLineAsserted,
            None,
            None,
            None,
            "asserted",
        );
    }

    pub(crate) fn irq_line_deasserted(&mut self) {
        self.push(
            TraceEventKind::IrqLineDeasserted,
            None,
            None,
            None,
            "deasserted",
        );
    }

    pub(crate) fn irq_message_emitted(&mut self, source: IrqSource) {
        self.push(
            TraceEventKind::IrqMessageEmitted,
            None,
            irq_queue(source),
            None,
            irq_detail(source),
        );
    }

    pub(crate) fn irq_device_ack(&mut self, source: IrqSource) {
        self.push(
            TraceEventKind::IrqDeviceAck,
            None,
            irq_queue(source),
            None,
            irq_detail(source),
        );
    }
}

const fn irq_queue(source: IrqSource) -> Option<QueueId> {
    match source {
        IrqSource::Completion(_) => None,
        IrqSource::Event(queue_id) => Some(queue_id),
    }
}

const fn irq_detail(source: IrqSource) -> &'static str {
    match source {
        IrqSource::Completion(_) => "completion IRQ",
        IrqSource::Event(_) => "event IRQ",
    }
}
