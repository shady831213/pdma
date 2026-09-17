use super::{Driver, Hardware, Shared};
use hardware_interface_pipeline::{AsyncInteractionBackend, StepOutcome};
use pdma_model_dma::Device;
use std::convert::Infallible;

impl<M> Shared<M> {
    pub(super) async fn access<T>(&self, access: impl FnOnce(&mut Hardware<M>) -> T) -> T {
        let mut hardware = self.hardware.lock().await;
        let result = access(&mut hardware);
        if hardware.control.take_request_notification() {
            self.to_firmware.notify();
        }
        if hardware.control.take_response_notification() {
            self.to_host.notify();
        }
        if hardware.dma.take_interrupt() {
            self.interrupt.raise();
        }
        drop(hardware);
        self.changed.notify_one();
        result
    }
}

impl<M: Send> AsyncInteractionBackend<Device<M>, ()> for Driver<M> {
    type Error = Infallible;

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepOutcome<O, E, ()>, Infallible>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Device<M>) -> StepOutcome<O, E, ()> + Send,
    {
        tokio::task::yield_now().await;
        let mut hardware = self.0.hardware.lock().await;
        let outcome = interaction(&mut hardware.dma);
        if hardware.dma.take_interrupt() {
            self.0.interrupt.raise();
        }
        drop(hardware);
        Ok(outcome)
    }

    async fn wait(&self, (): ()) -> Result<(), Infallible> {
        self.0.changed.notified().await;
        Ok(())
    }
}
