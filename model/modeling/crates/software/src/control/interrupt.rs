use hardware_interface_irq::{InterruptEnvelope, InterruptHandler};
use hardware_interface_software::{NotificationWait, Notifications, Notifier, notifications};
use std::convert::Infallible;

/// One management interrupt destination on the firmware CPU.
#[derive(Clone, Copy, Debug)]
pub struct ControlIrq;

impl InterruptEnvelope for ControlIrq {
    type Vector = ();
    fn vector(&self) {}
}

/// Software work notification raised only by its registered management ISR.
#[derive(Clone, Debug)]
pub struct InterruptEvent {
    sender: Notifier,
    receiver: Notifications,
}

impl InterruptEvent {
    /// Creates the work event before software installs its interrupt handler.
    pub fn new() -> Self {
        let (sender, receiver) = notifications();
        Self { sender, receiver }
    }

    /// Supplies the ISR for registration on the receiving software CPU.
    pub fn handler(&self) -> ControlIsr {
        ControlIsr(self.sender.clone())
    }

    /// Waits for ISR delivery; callers recheck ring state after every wake.
    pub async fn wait(&self) {
        self.receiver.wait_notification().await;
    }
}

impl Default for InterruptEvent {
    fn default() -> Self {
        Self::new()
    }
}

/// A management ISR that wakes work without executing management requests.
#[derive(Debug)]
pub struct ControlIsr(Notifier);

impl<N: InterruptEnvelope> InterruptHandler<N> for ControlIsr {
    type Error = Infallible;
    async fn handle(&mut self, _: &N) -> Result<(), Infallible> {
        self.0.notify();
        Ok(())
    }
}
