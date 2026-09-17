use std::{future::Future, sync::Arc};
use tokio::sync::Notify;

/// Waits for a coalesced progress hint; callers must recheck their shared state.
pub trait NotificationWait: Sync {
    /// Waits until a notification is available.
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
#[derive(Debug)]
pub struct Notifications(Arc<Notify>);

impl NotificationWait for Notifications {
    async fn wait_notification(&self) {
        self.0.notified().await;
    }
}

/// Creates a sender and its software notification destination.
pub fn notifications() -> (Notifier, Notifications) {
    let notify = Arc::new(Notify::new());
    (Notifier(Arc::clone(&notify)), Notifications(notify))
}
