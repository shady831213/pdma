use hardware_interface_runtime::InterruptSource;
use pdma_model_host::runtime::Irq;
use tokio::sync::watch;

/// Platform-owned coalesced notification state for one software interrupt domain.
#[derive(Clone, Debug)]
pub struct Notifications {
    pending: watch::Sender<bool>,
}

impl Notifications {
    pub(super) fn new() -> Self {
        let (pending, _) = watch::channel(false);
        Self { pending }
    }

    pub(super) fn publish(&self, emitted: bool) {
        if emitted {
            self.pending.send_if_modified(|pending| {
                let changed = !*pending;
                *pending = true;
                changed
            });
        }
    }
}

impl InterruptSource for Notifications {
    type Notification = Irq;
    type Ready = ();
    type Error = watch::error::RecvError;

    async fn wait_ready(&self) -> Result<(), Self::Error> {
        self.pending
            .subscribe()
            .wait_for(|pending| *pending)
            .await?;
        Ok(())
    }

    async fn claim(&mut self, (): ()) -> Result<Option<Irq>, Self::Error> {
        Ok(self.pending.send_replace(false).then_some(Irq))
    }
}
