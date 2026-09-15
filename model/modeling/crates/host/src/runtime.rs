//! PDMA-specific runtime transport; not part of the software channel driver.

#[cfg(test)]
mod tests;

use hardware_interface_pipeline::{AsyncInteractionBackend, StepOutcome};
use hardware_interface_runtime::{InterruptEnvelope, InterruptSource, RegisterAccess};
use pdma_model_dma::{Device, Error};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::{Mutex, Notify, watch};

/// Shared async transport for one device, driven by one retained worker operation.
#[derive(Debug)]
pub struct Model<M> {
    device: Arc<Mutex<Device<M>>>,
    changed: Arc<Notify>,
    pending: watch::Sender<bool>,
}

impl<M> Clone for Model<M> {
    fn clone(&self) -> Self {
        Self {
            device: Arc::clone(&self.device),
            changed: Arc::clone(&self.changed),
            pending: self.pending.clone(),
        }
    }
}

impl<M> Model<M> {
    /// Wraps externally assembled hardware without selecting its memory backend.
    pub fn new(mut device: Device<M>) -> Self {
        let (pending, _) = watch::channel(device.take_interrupt());
        Self {
            device: Arc::new(Mutex::new(device)),
            changed: Arc::new(Notify::new()),
            pending,
        }
    }

    /// Reports an emitted notification awaiting platform dispatch, not a hardware level.
    pub fn irq(&self) -> bool {
        *self.pending.borrow()
    }

    fn publish_irq(&self, emitted: bool) {
        if emitted {
            self.pending.send_if_modified(|pending| {
                let changed = !*pending;
                *pending = true;
                changed
            });
        }
    }
}

impl<M: Send> Model<M> {
    async fn access(
        &self,
        access: impl FnOnce(&mut Device<M>) -> Result<(), Error> + Send,
    ) -> Result<(), Error> {
        let result = {
            let mut device = self.device.lock().await;
            let result = access(&mut device);
            self.publish_irq(device.take_interrupt());
            result
        };
        self.changed.notify_one();
        result
    }
}

impl<M: Send> RegisterAccess for Model<M> {
    type Error = Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Error> {
        let offset = usize::try_from(offset).map_err(|_| Error::Unmapped)?;
        self.access(|device| device.read(offset, response)).await
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Error> {
        let offset = usize::try_from(offset).map_err(|_| Error::Unmapped)?;
        self.access(|device| device.write(offset, request)).await
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
        self.publish_irq(device.take_interrupt());
        drop(device);
        Ok(outcome)
    }

    async fn wait(&self, (): ()) -> Result<(), Infallible> {
        self.changed.notified().await;
        Ok(())
    }
}

/// Notification for the device's shared edge/message interrupt vector.
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
        let mut pending = self.pending.subscribe();
        pending.wait_for(|ready| *ready).await?;
        Ok(())
    }

    async fn claim(&mut self, (): ()) -> Result<Option<Irq>, Self::Error> {
        Ok(self.pending.send_replace(false).then_some(Irq))
    }
}
