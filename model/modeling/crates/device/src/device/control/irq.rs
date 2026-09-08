use super::DeviceCore;
use crate::ModelResult;
use pdma_model_contract::semantic::{EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot};

impl DeviceCore {
    pub(crate) fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.irq
            .enable_source(source, &self.queues, &mut self.trace)
    }

    pub(crate) fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.irq.configure_source_route(source, route, &self.queues)
    }

    pub(crate) fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.irq
            .disable_source(source, &self.queues, &mut self.trace)
    }

    pub(crate) fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.irq.mask_source(source, &self.queues, &mut self.trace)
    }

    pub(crate) fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.irq
            .unmask_source(source, &self.queues, &mut self.trace)
    }

    pub(crate) fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.irq.ack_source(source, &self.queues, &mut self.trace)
    }

    pub(crate) fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.irq.pending_snapshot()
    }

    pub(crate) fn take_irq_notification(&mut self) -> Option<IrqNotification> {
        self.irq.take_traced_notification(&mut self.trace)
    }

    pub(crate) fn irq_notification_ready(&self) -> bool {
        self.irq.notification_ready()
    }
}
