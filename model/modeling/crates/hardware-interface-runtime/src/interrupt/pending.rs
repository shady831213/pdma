use super::{InterruptEnvelope, InterruptSource};
use tokio::sync::watch;

/// Creates one initially idle, coalescing interrupt destination.
///
/// Every successful claim clones the supplied envelope. This is a fixed
/// destination, not a queue or a transport for per-raise payloads.
pub fn pending_interrupt<N: InterruptEnvelope + Clone>(
    notification: N,
) -> (InterruptEmitter, PendingInterrupt<N>) {
    let (pending, _) = watch::channel(false);
    (
        InterruptEmitter {
            pending: pending.clone(),
        },
        PendingInterrupt {
            pending,
            notification,
        },
    )
}

/// Cloneable permission to raise one destination, without permission to claim it.
///
/// ```compile_fail,E0599
/// use hardware_interface_runtime::{InterruptEmitter, InterruptSource};
/// async fn cannot_claim(mut emitter: InterruptEmitter) {
///     emitter.claim(()).await;
/// }
/// ```
#[derive(Clone, Debug)]
pub struct InterruptEmitter {
    pending: watch::Sender<bool>,
}

impl InterruptEmitter {
    /// Sets pending state; repeated raises before a claim coalesce.
    pub fn raise(&self) {
        self.pending.send_if_modified(|pending| {
            let changed = !*pending;
            *pending = true;
            changed
        });
    }
}

/// Claim capability for one fixed, coalescing interrupt destination.
///
/// Clones share pending state: multiple waits may become ready, but only one
/// claim consumes each pending notification. Waiting is non-consuming and
/// cancellation-safe. Claim clears atomically, so raises after it survive.
/// The source keeps its channel alive; dropping all emitters does not signal
/// end-of-stream. Dispatcher shutdown remains explicit.
///
/// ```compile_fail,E0599
/// use hardware_interface_runtime::{InterruptEnvelope, PendingInterrupt};
/// fn cannot_raise<N: InterruptEnvelope + Clone>(source: PendingInterrupt<N>) {
///     source.raise();
/// }
/// ```
#[derive(Clone, Debug)]
pub struct PendingInterrupt<N> {
    pending: watch::Sender<bool>,
    notification: N,
}

impl<N> PendingInterrupt<N> {
    /// Reports whether a notification is currently awaiting a claim.
    pub fn is_pending(&self) -> bool {
        *self.pending.borrow()
    }
}

impl<N: InterruptEnvelope + Clone> InterruptSource for PendingInterrupt<N> {
    type Notification = N;
    type Ready = ();
    type Error = watch::error::RecvError;

    async fn wait_ready(&self) -> Result<(), Self::Error> {
        let mut pending = self.pending.subscribe();
        pending.wait_for(|ready| *ready).await?;
        Ok(())
    }

    async fn claim(&mut self, (): ()) -> Result<Option<N>, Self::Error> {
        Ok(self
            .pending
            .send_replace(false)
            .then(|| self.notification.clone()))
    }
}
