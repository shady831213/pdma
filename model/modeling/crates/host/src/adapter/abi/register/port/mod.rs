mod api;
mod bootstrap;
mod control;
mod descriptor;
mod driver;
mod irq;
mod pending;
mod registers;
mod retirement;
mod state;
mod submission;

use super::{
    PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim, PdmaMemoryProvisioning,
    PdmaProvisioningError, PdmaRegisterAccessError, PdmaRegisterPortError, RegisterQueueStatus,
};
use crate::PortError;
use hardware_interface_runtime::{CpuLocal, MemoryAccess, RegisterAccess};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue, IrqNotification};
use std::{future::Future, future::poll_fn, sync::Arc, task::Poll};
use tokio::sync::Mutex;

type PdmaPortResult<T> = Result<T, PdmaRegisterPortError>;

/// CPU-bound canonical PDMA port over split capabilities.
///
/// # Cancel safety
///
/// Accesses execute in order; completed allocations, memory writes and register
/// writes persist when a future is dropped. Clones resolve an issued queue-command
/// response before issuing another command, without reissuing the original command.
/// Unresolved issued submissions retain driver credit even if every port is dropped.
/// Retrying creation with the same configuration recovers an accepted activation;
/// retirement retries deliver each retained record once. Reset requires a new driver.
#[derive(Debug, Clone)]
pub struct PdmaRegisterPort<A> {
    access: A,
    state: CpuLocal<state::PortState>,
    queue_window: Arc<Mutex<()>>,
    irq_window: Arc<Mutex<()>>,
}

impl<A: CanonicalAccess> PdmaRegisterPort<A> {
    /// Captures the active CPU and starts at the initial device epoch.
    pub fn new(access: A) -> PdmaPortResult<Self> {
        Self::try_new_at(access, DeviceEpoch::initial())
    }
    /// Captures the active CPU at an explicit device epoch.
    pub fn new_at(access: A, epoch: DeviceEpoch) -> PdmaPortResult<Self> {
        Self::try_new_at(access, epoch)
    }
    /// Captures the active CPU while preserving its typed failure.
    pub fn try_new_at(access: A, epoch: DeviceEpoch) -> PdmaPortResult<Self> {
        Ok(Self {
            access,
            state: CpuLocal::capture_active(state::PortState::new(epoch))
                .map_err(PdmaRegisterPortError::from)?,
            queue_window: Arc::new(Mutex::new(())),
            irq_window: Arc::new(Mutex::new(())),
        })
    }
    /// Returns the clone-shared split capability.
    pub const fn access(&self) -> &A {
        &self.access
    }
    /// Reads one queue status through this port's clone-shared selector state.
    pub async fn queue_status(&self, queue: EpochQueue) -> PdmaPortResult<RegisterQueueStatus> {
        self.try_queue_status(queue).await
    }
    /// Reads one queue status without erasing register or response failures.
    pub async fn try_queue_status(&self, queue: EpochQueue) -> PdmaPortResult<RegisterQueueStatus> {
        let _window = self.queue_window.lock().await;
        let state = self.with_state(|state| {
            state.queue.ok_or_else(|| {
                PdmaRegisterPortError::from(PortError::UnknownQueue(queue.queue_id()))
            })
        })??;
        if state.status.queue() != queue {
            return Err(PortError::UnknownQueue(queue.queue_id()).into());
        }
        control::read_status(self, queue, state.rings).await
    }
    fn with_state<R>(
        &self,
        operation: impl FnOnce(&mut state::PortState) -> R,
    ) -> PdmaPortResult<R> {
        self.state
            .with(operation)
            .map_err(PdmaRegisterPortError::from)
    }
    fn validate_owner(&self) -> PdmaPortResult<()> {
        self.with_state(|_| ())
    }
    async fn poll_typed<F, T, E>(&self, future: F) -> PdmaPortResult<T>
    where
        F: Future<Output = Result<T, E>> + Send,
        T: Send,
        E: Send,
        PdmaRegisterPortError: From<E>,
    {
        let mut future = Box::pin(future);
        poll_fn(|context| {
            if let Err(error) = self.validate_owner() {
                return Poll::Ready(Err(error));
            }
            match future.as_mut().poll(context) {
                Poll::Ready(result) => Poll::Ready(result.map_err(PdmaRegisterPortError::from)),
                Poll::Pending => Poll::Pending,
            }
        })
        .await
    }
    async fn poll_provision<F, T>(&self, future: F) -> PdmaPortResult<T>
    where
        F: Future<Output = Result<T, PdmaProvisioningError>> + Send,
        T: Send,
    {
        self.poll_typed(future).await
    }
    async fn poll_register<F, T>(&self, future: F) -> PdmaPortResult<T>
    where
        F: Future<Output = Result<T, PdmaRegisterAccessError>> + Send,
        T: Send,
    {
        self.poll_typed(future).await
    }
    async fn poll_claim<F, T>(&self, future: F) -> PdmaPortResult<T>
    where
        F: Future<Output = Result<T, PdmaClaimError>> + Send,
        T: Send,
    {
        self.poll_typed(future).await
    }
    async fn claim_notification(&self) -> PdmaPortResult<Option<IrqNotification>> {
        loop {
            match self.poll_claim(self.access.claim_interrupt_exact()).await? {
                PdmaClaimResult::Notification { notification, .. } => {
                    return Ok(Some(notification));
                }
                PdmaClaimResult::Empty => {}
                PdmaClaimResult::Shutdown => return Ok(None),
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
        + PdmaExactInterruptClaim<Error = PdmaClaimError>
{
}
