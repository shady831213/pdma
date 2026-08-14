use crate::ModelError;
use crate::ModelResult;
mod poll_irq;
mod state;
mod submission;

use super::AsyncDevicePort;
use crate::{AsyncPdmaBus, CpuLocal};
use pdma_model_contract::{
    BufferId, CompletionRecord, EpochIrqSource, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};
use pdma_model_contract::{DeviceEpoch, EpochQueue};
use pdma_model_device::{BusControlError, PdmaMemoryError};
use state::PortState;
use std::future::{Future, poll_fn};

/// Adapts a transport DMA error into the closed host model error vocabulary.
pub trait BusDmaErrorAdapter {
    /// Preserves the typed meaning of a failed DMA operation.
    fn into_model_error(self, buffer: Option<BufferId>) -> ModelError;
}

/// Adapts a transport control error into the closed host model error vocabulary.
pub trait BusControlErrorAdapter {
    /// Preserves stale-epoch and queue identity meaning for the failed operation.
    fn into_model_error(self, queue: EpochQueue) -> ModelError;
}

impl BusDmaErrorAdapter for PdmaMemoryError {
    fn into_model_error(self, buffer: Option<BufferId>) -> ModelError {
        match self {
            Self::AddressExhausted | Self::StorageExhausted => ModelError::DmaAllocationExhausted,
            Self::Unmapped => {
                buffer.map_or(ModelError::DmaSubmissionState, ModelError::UnknownBuffer)
            }
            Self::CrossesRegion
            | Self::OverlappingRegion
            | Self::InvalidRegion
            | Self::StaleStage { .. } => ModelError::DmaSubmissionState,
        }
    }
}

impl BusControlErrorAdapter for BusControlError {
    fn into_model_error(self, queue: EpochQueue) -> ModelError {
        match self {
            Self::StaleEpoch { expected, actual } => {
                ModelError::StaleDeviceEpoch { expected, actual }
            }
            Self::InvalidQueue => ModelError::UnknownQueue(queue.queue_id()),
            Self::Dma(error) => error.into_model_error(None),
            Self::Abi(_) | Self::InvalidTransaction => ModelError::DmaSubmissionState,
        }
    }
}

/// CPU-bound clone-shared host port that drives PDMA through the fixed-width ABI bus.
#[derive(Debug, Clone)]
pub struct BusDevicePort<B> {
    bus: B,
    state: CpuLocal<PortState>,
}

impl<B: Sync> BusDevicePort<B> {
    /// Captures the active software CPU as this port's owner.
    pub fn new(bus: B) -> ModelResult<Self> {
        Self::new_at(bus, DeviceEpoch::initial())
    }

    /// Captures the active software CPU for an explicitly known current device epoch.
    pub fn new_at(bus: B, epoch: DeviceEpoch) -> ModelResult<Self> {
        Ok(Self {
            bus,
            state: CpuLocal::capture_active(PortState::new(epoch))?,
        })
    }

    pub(crate) fn validate_owner(&self) -> ModelResult<()> {
        self.state.with(|_| ())
    }

    async fn poll_model<F, T>(&self, future: F) -> ModelResult<T>
    where
        F: Future<Output = ModelResult<T>> + Send,
        T: Send,
    {
        let mut future = Box::pin(future);
        poll_fn(|context| {
            if let Err(error) = self.state.with(|_| ()) {
                return std::task::Poll::Ready(Err(error));
            }
            future.as_mut().poll(context)
        })
        .await
    }

    async fn poll_dma<F, T, E>(&self, future: F, buffer: Option<BufferId>) -> ModelResult<T>
    where
        F: Future<Output = Result<T, E>> + Send,
        T: Send,
        E: BusDmaErrorAdapter + Send,
    {
        let mut future = Box::pin(future);
        poll_fn(|context| {
            if let Err(error) = self.state.with(|_| ()) {
                return std::task::Poll::Ready(Err(error));
            }
            match future.as_mut().poll(context) {
                std::task::Poll::Ready(result) => {
                    std::task::Poll::Ready(result.map_err(|error| error.into_model_error(buffer)))
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        })
        .await
    }

    async fn poll_control<F, T, E>(&self, future: F, queue: EpochQueue) -> ModelResult<T>
    where
        F: Future<Output = Result<T, E>> + Send,
        T: Send,
        E: BusControlErrorAdapter + Send,
    {
        let mut future = Box::pin(future);
        poll_fn(|context| {
            if let Err(error) = self.state.with(|_| ()) {
                return std::task::Poll::Ready(Err(error));
            }
            match future.as_mut().poll(context) {
                std::task::Poll::Ready(result) => {
                    std::task::Poll::Ready(result.map_err(|error| error.into_model_error(queue)))
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        })
        .await
    }
}

impl<B> AsyncDevicePort for BusDevicePort<B>
where
    B: AsyncPdmaBus + Clone,
    B::DmaError: BusDmaErrorAdapter,
    B::ControlError: BusControlErrorAdapter,
{
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        state::create_queue(self, config).await
    }

    async fn allocate_buffer(&mut self, len: usize) -> ModelResult<BufferId> {
        state::allocate_buffer(self, len).await
    }

    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        state::write_buffer(self, id, offset, data).await
    }

    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        state::read_buffer(self, id, offset, len).await
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        submission::submit(self, queue, command).await
    }

    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        submission::doorbell(self, queue).await
    }

    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        poll_irq::completion(self, queue).await
    }

    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        poll_irq::event(self, queue).await
    }

    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        poll_irq::configure(self, source, route).await
    }

    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        poll_irq::enable(self, source).await
    }

    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        poll_irq::disable(self, source).await
    }

    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        poll_irq::mask(self, source).await
    }

    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        poll_irq::unmask(self, source).await
    }

    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        poll_irq::ack(self, source).await
    }

    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.bus.irq_snapshot().await
    }

    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        self.bus.wait_irq().await
    }

    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.bus.wait_irq_notification().await
    }
}
