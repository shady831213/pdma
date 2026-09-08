use super::DeviceCore;
use pdma_model_contract::semantic::{IrqNotification, IrqSnapshot};
impl DeviceCore {
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
