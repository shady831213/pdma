use super::{CanonicalAccess, PdmaRegisterPort, irq, retirement, state, submission};
use crate::{DriverPort, PortError, PortResult};
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, EpochIrqSource, EventRecord, IrqRoute, IrqSnapshot,
};

use crate::{Command, QueueConfig, QueueConfigHandle};
use pdma_model_contract::abi::registers::IrqSourceRegisters;

impl<A: CanonicalAccess> DriverPort for PdmaRegisterPort<A> {
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
        irq::configure(self, source, route)
            .await
            .map_err(PortError::from)
    }
    async fn enable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq::write_source(self, source, (IrqSourceRegisters::ENABLED.offset(), 1))
            .await
            .map_err(PortError::from)
    }
    async fn disable_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq::write_source(self, source, (IrqSourceRegisters::ENABLED.offset(), 0))
            .await
            .map_err(PortError::from)
    }
    async fn mask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq::write_source(self, source, (IrqSourceRegisters::MASKED.offset(), 1))
            .await
            .map_err(PortError::from)
    }
    async fn unmask_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq::write_source(self, source, (IrqSourceRegisters::MASKED.offset(), 0))
            .await
            .map_err(PortError::from)
    }
    async fn ack_irq_source(&mut self, source: EpochIrqSource) -> PortResult<()> {
        irq::write_source(self, source, (IrqSourceRegisters::ACK.offset(), 1))
            .await
            .map_err(PortError::from)
    }
    async fn irq_snapshot(&self) -> PortResult<Option<IrqSnapshot>> {
        irq::snapshot(self).await.map_err(PortError::from)
    }
}
