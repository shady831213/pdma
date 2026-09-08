use super::{CanonicalAccess, PdmaRegisterPort, irq, retirement, state, submission};
use crate::{DriverPort, PortError, PortResult};
use pdma_model_contract::semantic::{
    BufferId, Command, CompletionRecord, EpochIrqSource, EventRecord, IrqNotification, IrqRoute,
    IrqSnapshot, QueueConfig, QueueConfigHandle,
};

impl<A: CanonicalAccess> DriverPort for PdmaRegisterPort<A> {
    async fn recover_queue(&mut self, queue: QueueConfigHandle) -> PortResult<()> {
        let _window = self.queue_window.lock().await;
        self.with_state(|state| state.queue(queue))
            .map_err(PortError::from)??;
        super::pending::recover(self).await.map_err(PortError::from)
    }
    async fn create_queue(&mut self, config: QueueConfig) -> PortResult<QueueConfigHandle> {
        state::create_queue(self, config)
            .await
            .map_err(PortError::from)
    }
    async fn allocate_buffer(&mut self, len: usize) -> PortResult<BufferId> {
        self.try_allocate_buffer(len).await.map_err(PortError::from)
    }
    async fn write_buffer(&mut self, id: BufferId, offset: usize, data: &[u8]) -> PortResult<()> {
        state::write_buffer(self, id, offset, data).map_err(PortError::from)
    }
    async fn read_buffer(&self, id: BufferId, offset: usize, len: usize) -> PortResult<Vec<u8>> {
        state::read_buffer(self, id, offset, len).map_err(PortError::from)
    }
    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: Command,
    ) -> PortResult<()> {
        submission::submit(self, queue, command, None)
            .await
            .map_err(PortError::from)
    }
    async fn submit_command_tracked(
        &mut self,
        queue: QueueConfigHandle,
        command: Command,
        receipt: crate::semantic::SubmissionReceipt,
    ) -> PortResult<()> {
        submission::submit(self, queue, command, Some(receipt))
            .await
            .map_err(PortError::from)
    }
    async fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> PortResult<()> {
        submission::doorbell(self, queue)
            .await
            .map_err(PortError::from)
    }
    async fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> PortResult<Option<CompletionRecord>> {
        retirement::completion(self, queue)
            .await
            .map_err(PortError::from)
    }
    async fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> PortResult<Option<EventRecord>> {
        retirement::event(self, queue)
            .await
            .map_err(PortError::from)
    }
    async fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> PortResult<()> {
        irq_command(
            self,
            source,
            Some(route),
            pdma_model_contract::abi::mmio::IrqCommand::ConfigureRoute,
        )
        .await
    }
    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Enable,
        )
        .await
    }
    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Disable,
        )
        .await
    }
    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Mask,
        )
        .await
    }
    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq_command(
            self,
            source,
            None,
            pdma_model_contract::abi::mmio::IrqCommand::Unmask,
        )
        .await
    }
    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
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
) -> PortResult<()> {
    irq::source_command(port, source, route, command)
        .await
        .map_err(PortError::from)
}
