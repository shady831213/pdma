use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, CompletionRingId, CompletionStatus, DeviceEpoch, QueueId,
};
use pdma_model_host::{Command, QueueConfig, QueueConfigHandle, QueueDirectoryEntry};
use pdma_model_host::{DriverPort, PortError};
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
    async fn create_queue(&mut self, config: QueueConfig) -> Result<QueueConfigHandle, PortError> {
        let queue = QueueId::new(self.queues);
        self.queues += 1;
        Ok(QueueConfigHandle::new(
            DeviceEpoch::initial(),
            QueueDirectoryEntry::new(queue, CompletionRingId::new(queue.raw()), config),
        ))
    }

    async fn allocate_buffer(&mut self, _len: usize) -> Result<BufferId, PortError> {
        let buffer = BufferId::new(self.allocations);
        self.allocations += 1;
        Ok(buffer)
    }

    async fn write_buffer(
        &mut self,
        _id: BufferId,
        _offset: usize,
        _data: &[u8],
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn read_buffer(
        &self,
        id: BufferId,
        _offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, PortError> {
        if id.raw() >= self.allocations {
            Err(PortError::UnknownBuffer(id))
        } else {
            Ok(vec![0; len])
        }
    }

    async fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: Command,
    ) -> Result<(), PortError> {
        if self.submit_blocked {
            pending::<()>().await;
        }
        self.submissions += 1;
        self.pending.push_back(CompletionRecord::new(
            queue.queue_id(),
            command.command_id(),
            CompletionStatus::Success,
        ));
        Ok(())
    }

    async fn ring_doorbell(&mut self, _queue: QueueConfigHandle) -> Result<(), PortError> {
        if self.doorbell_blocked {
            pending::<()>().await;
        }
        Ok(())
    }

    async fn poll_completion(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<CompletionRecord>, PortError> {
        let completion = self.pending.pop_front();
        self.completions += usize::from(completion.is_some());
        Ok(completion)
    }

    async fn poll_queue_event(
        &mut self,
        _queue: QueueConfigHandle,
    ) -> Result<Option<pdma_model_contract::semantic::EventRecord>, PortError> {
        Ok(None)
    }

    async fn configure_irq_route(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
        _route: pdma_model_contract::semantic::IrqRoute,
    ) -> Result<(), PortError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn enable_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), PortError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn disable_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), PortError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn mask_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), PortError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn unmask_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), PortError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn ack_irq_source(
        &mut self,
        _source: pdma_model_contract::semantic::EpochIrqSource,
    ) -> Result<(), PortError> {
        self.irq_calls += 1;
        Ok(())
    }

    async fn irq_snapshot(
        &self,
    ) -> Result<Option<pdma_model_contract::semantic::IrqSnapshot>, PortError> {
        Ok(None)
    }
}
