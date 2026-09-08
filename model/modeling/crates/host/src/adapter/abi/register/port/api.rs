use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, state};
use crate::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::semantic::{BufferId, EpochIrqSource, IrqRoute};

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
        super::irq::configure(self, source, route).await
    }
}
