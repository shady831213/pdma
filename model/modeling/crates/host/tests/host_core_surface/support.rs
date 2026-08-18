use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRecord, CompletionRingId, CompletionStatus, DeviceEpoch,
    ExternalCommand, QueueConfig, QueueConfigHandle, QueueDirectoryEntry, QueueId,
};
use pdma_model_host::{DriverPort, ModelError};
use std::future::pending;

use super::ProbePort;

impl ProbePort {
    pub(super) const fn block_submit(&mut self) {
        self.submit_blocked = true;
    }

    pub(super) const fn allow_submit(&mut self) {
        self.submit_blocked = false;
    }

    pub(super) const fn block_doorbell(&mut self) {
        self.doorbell_blocked = true;
    }
}

impl DriverPort for ProbePort {
    async fn create_queue(&mut self, config: QueueConfig) -> Result<QueueConfigHandle, ModelError> {
        let queue = QueueId::new(self.queues);
        self.queues += 1;
        Ok(QueueConfigHandle::new(
            DeviceEpoch::initial(),
            QueueDirectoryEntry::new(queue, CompletionRingId::new(queue.raw()), config),
        ))
    }

    async fn allocate_buffer(&mut self, _len: usize) -> Result<BufferId, ModelError> {
        let buffer = BufferId::new(self.allocations);
        self.allocations += 1;
        Ok(buffer)
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> Result<(), ModelError> {
        Ok(())
    }

    async fn read_buffer(
        &self,
        _id: BufferId,
        _offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, ModelError> {
        Ok(vec![0; len])
    }

    async fn submit_command(
        &mut self,
        _queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> Result<(), ModelError> {
        if self.submit_blocked {
            pending::<()>().await;
        }
        let (queue, command) = command_identity(command);
        self.submissions += 1;
        self.pending.push_back(CompletionRecord::new(
            queue,
            command,
            CompletionStatus::Success,
        ));
        Ok(())
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> Result<(), ModelError> {
        if self.doorbell_blocked {
            pending::<()>().await;
        }
        Ok(())
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<CompletionRecord>, ModelError> {
        let completion = self.pending.pop_front();
        self.completions += usize::from(completion.is_some());
        Ok(completion)
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<pdma_model_contract::semantic::EventRecord>, ModelError> {
        Ok(None)
    }

    async fn configure_irq_route(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
        _route: pdma_model_contract::semantic::IrqRoute,
    ) -> Result<(), ModelError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn enable_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), ModelError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn disable_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), ModelError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn mask_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), ModelError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn unmask_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), ModelError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn ack_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), ModelError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn irq_snapshot(&self) -> Option<pdma_model_contract::semantic::IrqSnapshot> {
        None
    }

    async fn wait_irq(&self) -> Option<pdma_model_contract::semantic::IrqSnapshot> {
        None
    }

    async fn wait_irq_notification(
        &mut self,
    ) -> Option<pdma_model_contract::semantic::IrqNotification> {
        None
    }
}

fn command_identity(command: ExternalCommand) -> (QueueId, CommandId) {
    match command {
        ExternalCommand::Copy(command) => (command.queue_id(), command.command_id()),
        ExternalCommand::StridedCopy(command) => (command.queue_id(), command.command_id()),
        ExternalCommand::BroadcastCopy(command) => (command.queue_id(), command.command_id()),
        ExternalCommand::VectorAdd(command) => (command.queue_id(), command.command_id()),
    }
}
