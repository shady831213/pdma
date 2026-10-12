//! PDMA-specific runtime transport; not part of the software channel driver.

#[cfg(test)]
mod tests;

use hardware_interface_pipeline::{AsyncInteractionBackend, StepResult};
use hardware_interface_runtime::{
    InterruptEmitter, InterruptEnvelope, InterruptSource, PendingInterrupt, RegisterAccess,
    pending_interrupt,
};
use pdma_model_dma::{Dma, Error};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::{Mutex, watch};

/// Shared async transport for one device, driven by one retained worker operation.
#[derive(Debug)]
pub struct Model<M> {
    device: Arc<Mutex<Dma<M>>>,
    emitter: InterruptEmitter,
    pending: PendingInterrupt<Irq>,
}

impl<M> Clone for Model<M> {
    fn clone(&self) -> Self {
        Self {
            device: Arc::clone(&self.device),
            emitter: self.emitter.clone(),
            pending: self.pending.clone(),
        }
    }
}

impl<M> Model<M> {
    /// Wraps externally assembled hardware without selecting its memory backend.
    pub fn new(mut device: Dma<M>) -> Self {
        let (emitter, pending) = pending_interrupt(Irq);
        if device.take_interrupt() {
            emitter.raise();
        }
        Self {
            device: Arc::new(Mutex::new(device)),
            emitter,
            pending,
        }
    }

    /// Reports an emitted notification awaiting platform dispatch, not a hardware level.
    pub fn irq(&self) -> bool {
        self.pending.is_pending()
    }

    fn publish_irq(&self, emitted: bool) {
        if emitted {
            self.emitter.raise();
        }
    }
}

impl<M: Send> Model<M> {
    async fn access(
        &self,
        access: impl FnOnce(&mut Dma<M>) -> Result<(), Error> + Send,
    ) -> Result<(), Error> {
        let mut device = self.device.lock().await;
        let result = access(&mut device);
        self.publish_irq(device.take_interrupt());
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

impl<M: Send> AsyncInteractionBackend<Dma<M>> for Model<M> {
    type Error = Infallible;

    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepResult<O, E>, Infallible>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Dma<M>) -> StepResult<O, E> + Send,
    {
        let mut device = self.device.lock().await;
        let outcome = interaction(&mut device);
        self.publish_irq(device.take_interrupt());
        drop(device);
        Ok(outcome)
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
        self.pending.wait_ready().await
    }

    async fn claim(&mut self, (): ()) -> Result<Option<Irq>, Self::Error> {
        self.pending.claim(()).await
    }
}
