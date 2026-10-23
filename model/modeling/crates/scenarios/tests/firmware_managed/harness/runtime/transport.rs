use super::{Driver, Hardware, RoutedMemory, Shared};
use hardware_interface_pipeline::{AsyncInteractionBackend, StepResult};
use pdma_model_contract::dma::DmaAccess;
use pdma_model_dma::Dma;
use std::{
    convert::Infallible,
    sync::atomic::{AtomicU64, Ordering},
};

/// Harness time advances per root hardware interaction, not per read or child stage.
#[derive(Debug, Default)]
pub(super) struct SimulationTimer(AtomicU64);

impl pdma_model_dma::Timer for SimulationTimer {
    fn sample(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

impl SimulationTimer {
    fn advance(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

impl<M> Shared<M> {
    pub(super) async fn access<T>(&self, access: impl FnOnce(&mut Hardware<M>) -> T) -> T {
        let mut hardware = self.hardware.lock().await;
        self.deliver(&mut hardware);
        let result = access(&mut hardware);
        self.deliver(&mut hardware);
        drop(hardware);
        result
    }

    fn deliver(&self, hardware: &mut Hardware<M>) {
        if let Some(contexts) = &hardware.contexts {
            contexts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .step(&hardware.dma);
        }
        let mut control = hardware
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if control.take_request_notification() {
            self.to_firmware.raise();
        }
        if control.take_response_notification() {
            self.to_host.raise();
        }
        if control.take_event_notification() {
            self.to_events.raise();
        }
        drop(control);
        for index in 0..self.channel_count {
            let channel = pdma_model_contract::dma::ChannelId::new(index);
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

impl<M: Send> AsyncInteractionBackend<Dma<RoutedMemory<M>, DmaAccess>> for Driver<M> {
    type Error = Infallible;

    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }

    async fn interact<O, E, F>(&self, interaction: F) -> Result<StepResult<O, E>, Infallible>
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut Dma<RoutedMemory<M>, DmaAccess>) -> StepResult<O, E> + Send,
    {
        let mut hardware = self.0.hardware.lock().await;
        self.0.timer.advance();
        let outcome = interaction(&mut hardware.dma);
        self.0.deliver(&mut hardware);
        drop(hardware);
        Ok(outcome)
    }
}
