use crate::queue::QueueBlock;
use crate::trace::TraceSink;
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochIrqSource, IrqCause, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
};

#[derive(Debug)]
pub(crate) struct PdmaIrqController {
    epoch: DeviceEpoch,
    snapshot: Option<IrqSnapshot>,
    selected: Option<IrqSource>,
}

impl PdmaIrqController {
    pub(crate) const fn new(epoch: DeviceEpoch) -> Self {
        Self {
            epoch,
            snapshot: None,
            selected: None,
        }
    }

    pub(crate) fn update(&mut self, queues: &QueueBlock, trace: &mut TraceSink) {
        let was_asserted = self.snapshot.is_some_and(IrqSnapshot::asserted);
        let mut pending = None;
        let mut asserted = None;
        let mut message = None;
        for (source, state) in queues.irq_sources() {
            match state.route() {
                IrqRoute::Line(line) if state.has_line_cause() => {
                    let snapshot = IrqSnapshot::new(
                        IrqCause::new(line, EpochIrqSource::new(self.epoch, source)),
                        state.asserts_line(),
                    );
                    pending.get_or_insert(snapshot);
                    if snapshot.asserted() {
                        asserted.get_or_insert(snapshot);
                    }
                }
                IrqRoute::Message(_) if state.can_deliver_message() => {
                    message.get_or_insert(source);
                }
                IrqRoute::Line(_) | IrqRoute::Message(_) => {}
            }
        }
        self.snapshot = asserted.or(pending);
        self.selected = asserted
            .map(|snapshot| snapshot.cause().source())
            .or(message);
        let is_asserted = asserted.is_some();
        match (was_asserted, is_asserted) {
            (false, true) => trace.irq_line_asserted(),
            (true, false) => trace.irq_line_deasserted(),
            (false, false) | (true, true) => {}
        }
    }

    pub(crate) fn pending_snapshot(&self, queues: &QueueBlock) -> Option<IrqSnapshot> {
        let snapshot = self.snapshot?;
        let source = queues.irq_source(snapshot.cause().source())?;
        source.has_line_cause().then(|| {
            IrqSnapshot::new(
                snapshot.cause(),
                snapshot.asserted() && source.asserts_line(),
            )
        })
    }

    pub(crate) fn notification_ready(&self, queues: &QueueBlock) -> bool {
        self.selected
            .and_then(|source| queues.irq_source(source))
            .is_some_and(|source| source.asserts_line() || source.can_deliver_message())
    }

    pub(crate) fn take_notification(&mut self, queues: &mut QueueBlock) -> Option<IrqNotification> {
        let selected = self.selected?;
        let source = queues.irq_source_mut(selected)?;
        match source.route() {
            IrqRoute::Line(line) => source.asserts_line().then(|| {
                IrqNotification::Line(IrqSnapshot::new(
                    IrqCause::new(line, EpochIrqSource::new(self.epoch, selected)),
                    true,
                ))
            }),
            IrqRoute::Message(_) => {
                let message = source.take_message(self.epoch, selected)?;
                self.selected = None;
                Some(IrqNotification::Message(message))
            }
        }
    }
}

#[cfg(test)]
mod tests;
