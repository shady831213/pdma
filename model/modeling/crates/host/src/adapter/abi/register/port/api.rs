use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, state};
use pdma_model_contract::semantic::{
    BufferId, EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot, QueueConfig,
    QueueConfigHandle,
};

impl<A: CanonicalAccess> PdmaRegisterPort<A> {
    /// Creates a queue without erasing provisioning or register failures.
    pub async fn try_create_queue(
        &mut self,
        config: QueueConfig,
    ) -> PdmaPortResult<QueueConfigHandle> {
        state::create_queue(self, config).await
    }

    /// Allocates one buffer without erasing provisioning failures.
    pub async fn try_allocate_buffer(&mut self, len: usize) -> PdmaPortResult<BufferId> {
        state::allocate_buffer(self, len).await
    }

    /// Configures a route without erasing register failures.
    pub async fn try_configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> PdmaPortResult<()> {
        super::irq::source_command(
            self,
            source,
            Some(route),
            pdma_model_contract::abi::mmio::IrqCommand::ConfigureRoute,
        )
        .await
    }

    /// Reads a line snapshot without erasing generated-response failures.
    pub async fn try_irq_snapshot(&self) -> PdmaPortResult<Option<IrqSnapshot>> {
        super::irq::snapshot(self).await
    }

    /// Claims a notification without erasing readiness or claim failures.
    pub async fn try_wait_irq_notification(&mut self) -> PdmaPortResult<Option<IrqNotification>> {
        self.claim_notification().await
    }
}
