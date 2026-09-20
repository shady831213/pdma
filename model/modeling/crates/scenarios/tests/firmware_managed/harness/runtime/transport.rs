use super::{Driver, Hardware, RoutedMemory, Shared};
use hardware_interface_pipeline::{AsyncInteractionBackend, StepOutcome};
use pdma_model_contract::ChannelId;
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
        if hardware.control.take_request_notification() {
            self.to_firmware.raise();
        }
        if hardware.control.take_response_notification() {
            self.to_host.raise();
        }
        for index in 0..self.channel_count {
            let channel = pdma_model_contract::ChannelId::new(index);
            if hardware.dma.take_channel_interrupt(channel) {
                let owner = hardware
                    .windows
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .owner(channel);
                if let Some(destination) = owner
                    .and_then(|function| usize::try_from(function.raw()).ok())
                    .and_then(|index| self.destinations.get(index))
                {
                    destination.raise();
                }
            }
        }
        if hardware.dma.take_device_interrupt() {
            self.to_firmware.raise();
        }
    }
}

impl<M: Send> AsyncInteractionBackend<Dma<RoutedMemory<M>, ChannelId>, ()> for Driver<M> {
    type Error = Infallible;

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepOutcome<O, E, ()>, Infallible>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Dma<RoutedMemory<M>, ChannelId>) -> StepOutcome<O, E, ()> + Send,
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
