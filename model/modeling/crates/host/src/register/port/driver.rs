use super::{CanonicalAccess, PdmaRegisterPort, irq, mapping, retirement, state, submission};
use crate::{DriverPort, ModelResult};
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, EpochIrqSource, EventRecord, ExternalCommand, IrqNotification,
    IrqRoute, IrqSnapshot, QueueConfig, QueueConfigHandle,
};

impl<A: CanonicalAccess> DriverPort for PdmaRegisterPort<A>
where
    A::Proposal: Send,
{
    async fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        state::create_queue(self, config)
            .await
            .map_err(|error| mapping::legacy(&error))
    }
    async fn allocate_buffer(&mut self, len: usize) -> ModelResult<BufferId> {
        self.try_allocate_buffer(len)
            .await
            .map_err(|error| mapping::legacy(&error))
    }
    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
        state::write_buffer(self, id, offset, data).map_err(|error| mapping::legacy(&error))
    }
    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
        state::read_buffer(self, id, offset, len).map_err(|error| mapping::legacy(&error))
    }
    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        submission::submit(self, queue, command)
            .await
            .map_err(|error| mapping::legacy(&error))
    }
    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        submission::doorbell(self, queue)
            .await
            .map_err(|error| mapping::legacy(&error))
    }
    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        retirement::completion(self, queue)
            .await
            .map_err(|error| mapping::legacy(&error))
    }
    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        retirement::event(self, queue)
            .await
            .map_err(|error| mapping::legacy(&error))
    }
    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        irq_command(
            self,
            source,
            Some(route),
            pdma_model_contract::abi::mmio::IrqCommand::ConfigureRoute,
        )
        .await
    }
    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Enable,
        )
        .await
    }
    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Disable,
        )
        .await
    }
    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Mask,
        )
        .await
    }
    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Unmask,
        )
        .await
    }
    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Acknowledge,
        )
        .await
    }
    async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        irq::snapshot(self).await.ok().flatten()
    }
    async fn wait_irq(&self) -> Option<IrqSnapshot> {
        irq::wait_line(self).await.ok().flatten()
    }
    async fn wait_irq_notification(&mut self) -> Option<IrqNotification> {
        self.claim_notification().await.ok().flatten()
    }
}

async fn irq_command<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    source: EpochIrqSource,
    route: Option<IrqRoute>,
    command: pdma_model_contract::abi::mmio::IrqCommand,
) -> ModelResult<()> {
    irq::source_command(port, source, route, command)
        .await
        .map_err(|error| mapping::legacy(&error))
}
