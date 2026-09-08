use super::DeviceCore;
use pdma_model_contract::semantic::{IrqNotification, IrqSnapshot};
impl DeviceCore {
    pub(crate) fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.irq.pending_snapshot(&self.queues)
    }

    pub(crate) fn take_irq_notification(&mut self) -> Option<IrqNotification> {
        let notification = self.irq.take_notification(&mut self.queues);
        if let Some(IrqNotification::Message(message)) = notification {
            self.trace.irq_message_emitted(message.source());
        }
        notification
    }

    pub(crate) fn irq_notification_ready(&self) -> bool {
        self.irq.notification_ready(&self.queues)
    }
}
