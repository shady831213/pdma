//! MMIO-backed transport adapter for the existing asynchronous PDMA bus.
use crate::ModelResult;

#[path = "mmio_device/dma.rs"]
mod dma;
#[path = "mmio_device/error.rs"]
mod error;
#[path = "mmio_device/irq.rs"]
mod irq;
#[path = "mmio_device/queue.rs"]
mod queue;
#[path = "mmio_device/recording.rs"]
mod recording;

pub(in crate::register) use error::MmioBusControlError;
pub(in crate::register) use recording::MmioDeviceTransport;

use super::AsyncPdmaBus;
use pdma_model_contract::abi::DmaAddress;
#[cfg(test)]
use pdma_model_contract::abi::mmio::IrqSourceStatus;
use pdma_model_contract::bridge::{PdmaMemoryRegion, QueueActivation};
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Clone-shared MMIO bus with one lock for each selector window.
#[derive(Debug)]
pub(in crate::register) struct MmioDeviceBus<T = pdma_model_device::AsyncDeviceHandle> {
    transport: T,
    queue_window: Arc<Mutex<QueueWindow>>,
    irq_window: Arc<Mutex<()>>,
}

#[derive(Debug, Default)]
struct QueueWindow {
    layouts: BTreeMap<EpochQueue, pdma_model_contract::bridge::QueueRingRegions>,
}

impl<T: Clone> Clone for MmioDeviceBus<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            queue_window: Arc::clone(&self.queue_window),
            irq_window: Arc::clone(&self.irq_window),
        }
    }
}

impl<T> MmioDeviceBus<T> {
    /// Wraps one raw device transport with clone-shared selector ownership.
    pub(in crate::register) fn new(transport: T) -> Self {
        Self {
            transport,
            queue_window: Arc::new(Mutex::new(QueueWindow::default())),
            irq_window: Arc::new(Mutex::new(())),
        }
    }
}

impl<T> AsyncPdmaBus for MmioDeviceBus<T>
where
    T: MmioDeviceTransport,
{
    type Stage = T::Stage;
    type DmaError = pdma_model_device::PdmaMemoryError;
    type ControlError = MmioBusControlError;

    async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        self.transport.allocate_memory_region(len).await
    }

    async fn stage_memory_regions(&self, lengths: &[u64]) -> Result<Self::Stage, Self::DmaError> {
        self.transport.stage_memory_regions(lengths).await
    }

    async fn read_dma(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, Self::DmaError> {
        self.transport.read_dma(address, length).await
    }

    async fn write_dma(&self, address: DmaAddress, bytes: &[u8]) -> Result<(), Self::DmaError> {
        self.transport.write_dma(address, bytes).await
    }

    async fn bootstrap_queue(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> Result<super::BusQueueStatus, Self::ControlError> {
        queue::bootstrap(self, stage, activation).await
    }

    async fn accept_submission(
        &self,
        request: super::BusSubmission<Self::Stage>,
    ) -> Result<super::BusQueueStatus, Self::ControlError> {
        queue::submit(self, request).await
    }

    async fn read_queue_status(
        &self,
        queue: EpochQueue,
    ) -> Result<super::BusQueueStatus, Self::ControlError> {
        queue::read_status(self, queue).await
    }

    async fn write_queue_control(
        &self,
        request: super::QueueControlWrite,
    ) -> Result<super::BusQueueStatus, Self::ControlError> {
        queue::write_control(self, request).await
    }

    async fn configure_irq_route(
        &self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        irq::source_command(self, source, Some(route), 1)
            .await
            .map(|_| ())
    }

    async fn enable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        irq::source_command(self, source, None, 2).await.map(|_| ())
    }

    async fn disable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        irq::source_command(self, source, None, 3).await.map(|_| ())
    }

    async fn mask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        irq::source_command(self, source, None, 4).await.map(|_| ())
    }

    async fn unmask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        irq::source_command(self, source, None, 5).await.map(|_| ())
    }

    async fn ack_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        irq::source_command(self, source, None, 6).await.map(|_| ())
    }

    #[cfg(test)]
    async fn observe_irq_source(&self, source: EpochIrqSource) -> ModelResult<IrqSourceStatus> {
        irq::source_command(self, source, None, 7).await
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        irq::snapshot(self).await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        irq::wait_line(self).await
    }

    async fn wait_irq_notification(&self) -> Option<IrqNotification> {
        irq::wait_notification(self).await
    }
}
