//! PDMA-specific runtime transport; not part of the software channel driver.

use hardware_interface_pipeline::{AsyncInteractionBackend, StepOutcome};
use hardware_interface_runtime::{InterruptEnvelope, InterruptSource, RegisterAccess};
use pdma_model_device::{Device, Error};
use std::{
    convert::Infallible,
    future::{Future, ready},
    sync::Arc,
};
use tokio::sync::{Mutex, Notify, watch};

/// Shared async transport for one device, driven by one retained worker operation.
#[derive(Debug)]
pub struct Model<M> {
    device: Arc<Mutex<Device<M>>>,
    changed: Arc<Notify>,
    level: watch::Sender<bool>,
}

impl<M> Clone for Model<M> {
    fn clone(&self) -> Self {
        Self {
            device: Arc::clone(&self.device),
            changed: Arc::clone(&self.changed),
            level: self.level.clone(),
        }
    }
}

impl<M> Model<M> {
    /// Wraps externally assembled hardware without selecting its memory backend.
    pub fn new(device: Device<M>) -> Self {
        let (level, _) = watch::channel(device.irq());
        Self {
            device: Arc::new(Mutex::new(device)),
            changed: Arc::new(Notify::new()),
            level,
        }
    }

    /// Returns the most recently committed hardware interrupt level.
    pub fn irq(&self) -> bool {
        *self.level.borrow()
    }

    fn publish_irq(&self, asserted: bool) {
        self.level.send_if_modified(|level| {
            let changed = *level != asserted;
            *level = asserted;
            changed
        });
    }
}

impl<M: Send> RegisterAccess for Model<M> {
    type Error = Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Error> {
        let offset = usize::try_from(offset).map_err(|_| Error::Unmapped)?;
        self.device.lock().await.read(offset, response)
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Error> {
        let offset = usize::try_from(offset).map_err(|_| Error::Unmapped)?;
        let result = {
            let mut device = self.device.lock().await;
            let result = device.write(offset, request);
            self.publish_irq(device.irq());
            result
        };
        self.changed.notify_one();
        result
    }
}

impl<M: Send> AsyncInteractionBackend<Device<M>, ()> for Model<M> {
    type Error = Infallible;

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepOutcome<O, E, ()>, Infallible>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Device<M>) -> StepOutcome<O, E, ()> + Send,
    {
        tokio::task::yield_now().await;
        let mut device = self.device.lock().await;
        let outcome = interaction(&mut device);
        self.publish_irq(device.irq());
        drop(device);
        Ok(outcome)
    }

    async fn wait(&self, (): ()) -> Result<(), Infallible> {
        self.changed.notified().await;
        Ok(())
    }
}

/// Notification for the channel's single level-sensitive interrupt vector.
#[derive(Clone, Copy, Debug)]
pub struct Irq;

impl InterruptEnvelope for Irq {
    type Vector = ();
    fn vector(&self) {}
}

impl<M: Send + 'static> InterruptSource for Model<M> {
    type Notification = Irq;
    type Ready = ();
    type Error = watch::error::RecvError;

    async fn wait_ready(&self) -> Result<(), Self::Error> {
        let mut level = self.level.subscribe();
        level.wait_for(|asserted| *asserted).await?;
        Ok(())
    }

    fn claim(&mut self, (): ()) -> impl Future<Output = Result<Option<Irq>, Self::Error>> + Send {
        ready(Ok(self.irq().then_some(Irq)))
    }
}
