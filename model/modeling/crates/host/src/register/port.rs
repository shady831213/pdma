#[path = "port/transport.rs"]
mod transport;

use self::transport::CanonicalTransport;
use super::bus::{BusQueueStatus, MmioDeviceBus};
use super::port::BusDevicePort;
use super::{
    PdmaBatchAccess, PdmaBatchError, PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim,
    PdmaMemoryProvisioning, PdmaProvisioningError, PdmaRegisterAccessError,
};
use crate::{DriverPort, ModelResult};
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::semantic::{
    CompletionRecord, DeviceEpoch, EpochIrqSource, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};
use std::{future::poll_fn, task::Poll};

/// CPU-bound canonical PDMA port over split capabilities.
#[derive(Debug, Clone)]
pub struct PdmaRegisterPort<A> {
    access: A,
    inner: BusDevicePort<MmioDeviceBus<CanonicalTransport<A>>>,
}

impl<A> PdmaRegisterPort<A>
where
    A: CanonicalAccess,
{
    /// Captures the active CPU and starts at the initial device epoch.
    pub fn new(access: A) -> ModelResult<Self> {
        Self::new_at(access, DeviceEpoch::initial())
    }

    /// Captures the active CPU at an explicit device epoch.
    pub fn new_at(access: A, epoch: DeviceEpoch) -> ModelResult<Self> {
        let transport = CanonicalTransport::new(access.clone());
        let inner = BusDevicePort::new_at(MmioDeviceBus::new(transport), epoch)?;
        Ok(Self { access, inner })
    }

    /// Returns the clone-shared split capability.
    pub const fn access(&self) -> &A {
        &self.access
    }

    /// Reads one queue status through this port's clone-shared selector state.
    pub async fn queue_status(
        &self,
        queue: pdma_model_contract::semantic::EpochQueue,
    ) -> ModelResult<BusQueueStatus> {
        self.inner.queue_status(queue).await
    }

    async fn claim_notification(&self) -> Option<IrqNotification> {
        loop {
            let mut future = Box::pin(self.access.claim_interrupt_exact());
            let result = poll_fn(|context| {
                if self.inner.validate_owner().is_err() {
                    return Poll::Ready(None);
                }
                match future.as_mut().poll(context) {
                    Poll::Ready(result) => Poll::Ready(Some(result)),
                    Poll::Pending => Poll::Pending,
                }
            })
            .await?;
            match result {
                Ok(PdmaClaimResult::Notification { notification, .. }) => {
                    return Some(notification);
                }
                Ok(PdmaClaimResult::Empty) => {}
                Ok(PdmaClaimResult::Shutdown) | Err(_) => return None,
            }
        }
    }
}

/// Complete canonical capability bound accepted by [`PdmaRegisterPort`].
pub trait CanonicalAccess:
    Clone
    + Send
    + Sync
    + MemoryAccess
    + RegisterAccess<Error = PdmaRegisterAccessError>
    + PdmaMemoryProvisioning<Error = PdmaProvisioningError>
    + PdmaBatchAccess<BatchError = PdmaBatchError>
    + PdmaExactInterruptClaim<Error = PdmaClaimError>
{
}

impl<T> CanonicalAccess for T where
    T: Clone
        + Send
        + Sync
        + MemoryAccess
        + RegisterAccess<Error = PdmaRegisterAccessError>
        + PdmaMemoryProvisioning<Error = PdmaProvisioningError>
        + PdmaBatchAccess<BatchError = PdmaBatchError>
        + PdmaExactInterruptClaim<Error = PdmaClaimError>
{
}

impl<A> DriverPort for PdmaRegisterPort<A>
where
    A: CanonicalAccess,
    A::Proposal: Send,
{
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.inner.create_queue(config).await
    }
    async fn allocate_buffer(
        &mut self,
        len: usize,
    ) -> ModelResult<pdma_model_contract::semantic::BufferId> {
        self.inner.allocate_buffer(len).await
    }
    async fn write_buffer(
        &mut self,
        id: pdma_model_contract::semantic::BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        self.inner.write_buffer(id, offset, data).await
    }
    async fn read_buffer(
        &self,
        id: pdma_model_contract::semantic::BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        self.inner.read_buffer(id, offset, len).await
    }
    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.inner.submit_command(queue, command).await
    }
    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.inner.ring_doorbell(queue).await
    }
    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.inner.poll_completion(queue).await
    }
    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.inner.poll_queue_event(queue).await
    }
    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.inner.configure_irq_route(source, route).await
    }
    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.inner.enable_irq_source(source).await
    }
    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.inner.disable_irq_source(source).await
    }
    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.inner.mask_irq_source(source).await
    }
    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.inner.unmask_irq_source(source).await
    }
    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.inner.ack_irq_source(source).await
    }
    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.inner.irq_snapshot().await
    }
    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        self.inner.wait_irq().await
    }
    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.claim_notification().await
    }
}
