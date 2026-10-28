use std::{future::Future, sync::Arc};
use tokio::sync::Notify;

/// Waits for a coalesced progress hint; callers must recheck their shared state.
pub trait NotificationWait: Sync {
    /// Consumes one coalesced progress hint, then returns so shared state can be rechecked.
    /// Unlike IRQ readiness waiting, this wait itself consumes the hint.
    fn wait_notification(&self) -> impl Future<Output = ()> + Send;
}

/// Sends progress hints to one software destination.
#[derive(Clone, Debug)]
pub struct Notifier(Arc<Notify>);

impl Notifier {
    /// Retains one hint when the destination is not waiting.
    pub fn notify(&self) {
        self.0.notify_one();
    }
}

/// Receives progress hints independently of interrupt dispatch.
#[derive(Clone, Debug)]
pub struct Notifications(Arc<Notify>);

impl NotificationWait for Notifications {
    async fn wait_notification(&self) {
        self.0.notified().await;
    }
}

/// Creates a sender and its software notification destination.
///
/// This pair carries coalesced progress hints, not counted events or interrupt
/// payloads. Publish shared state before notifying; a consumer checks its condition,
/// waits if necessary, then checks again. A wait consumes a hint, unlike IRQ
/// readiness observation, which needs a separate claim.
///
/// Clones share the same hint; delivery is not broadcast. Dropping senders does not
/// end waits, so the surrounding program must provide its own shutdown lifetime.
pub fn notifications() -> (Notifier, Notifications) {
    let notify = Arc::new(Notify::new());
    (Notifier(Arc::clone(&notify)), Notifications(notify))
}
