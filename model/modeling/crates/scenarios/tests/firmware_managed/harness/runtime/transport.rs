use super::{Driver, Hardware, Shared};
use hardware_interface_pipeline::{AsyncInteractionBackend, StepOutcome};
use pdma_model_dma::Dma;
use std::convert::Infallible;

impl<M> Shared<M> {
    pub(super) async fn access<T>(&self, access: impl FnOnce(&mut Hardware<M>) -> T) -> T {
        let mut hardware = self.hardware.lock().await;
        self.deliver(&mut hardware);
        let result = access(&mut hardware);
        self.deliver(&mut hardware);
        drop(hardware);
        self.changed.notify_one();
        result
    }

    fn deliver(&self, hardware: &mut Hardware<M>) {
        hardware
            .control
            .take_notifications(|function, request, response| {
                if request {
                    self.to_firmware.raise();
                }
                if response {
                    if let Some(destination) = usize::try_from(function.raw())
                        .ok()
                        .and_then(|index| self.destinations.get(index))
                    {
                        destination.to_host.raise();
                    }
                }
            });
        for index in 0..self.channel_count {
            let channel = pdma_model_contract::ChannelId::new(index);
            if hardware.dma.take_channel_interrupt(channel) {
                if let Some(destination) = hardware
                    .window
                    .interrupt_target(channel)
                    .and_then(|function| usize::try_from(function.raw()).ok())
                    .and_then(|index| self.destinations.get(index))
                {
                    destination.interrupt.raise();
                }
            }
        }
        if hardware.dma.take_device_interrupt() {
            self.to_firmware.raise();
        }
    }
}

impl<M: Send> AsyncInteractionBackend<Dma<M>, ()> for Driver<M> {
    type Error = Infallible;

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepOutcome<O, E, ()>, Infallible>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Dma<M>) -> StepOutcome<O, E, ()> + Send,
    {
        tokio::task::yield_now().await;
        let mut hardware = self.0.hardware.lock().await;
        let outcome = interaction(&mut hardware.dma);
        self.0.deliver(&mut hardware);
        drop(hardware);
        Ok(outcome)
    }

    async fn wait(&self, (): ()) -> Result<(), Infallible> {
        self.0.changed.notified().await;
        Ok(())
    }
}
