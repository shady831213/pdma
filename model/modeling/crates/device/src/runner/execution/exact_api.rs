use super::{DeviceExecution, QueueContext, io};
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{
    CompletionRingId, QueueConfig, QueueConfigHandle, QueueDirectoryEntry, QueueId,
};

pub(in crate::runner) struct PreparedExactQueue {
    context: QueueContext,
}

impl PreparedExactQueue {
    pub(in crate::runner) const fn queue_id(&self) -> QueueId {
        self.context.entry.queue_id()
    }
}

impl DeviceExecution {
    pub(in crate::runner) fn create_dma_bus_queue(
        &mut self,
        config: QueueConfig,
    ) -> ModelResult<QueueConfigHandle> {
        let prepared = self.prepare_dma_bus_queue(config)?;
        Ok(self.install_dma_bus_queue(prepared))
    }

    pub(in crate::runner) fn prepare_dma_bus_queue(
        &mut self,
        config: QueueConfig,
    ) -> ModelResult<PreparedExactQueue> {
        self.queues
            .try_reserve_exact(1)
            .map_err(|_| ModelError::DmaAllocationExhausted)?;
        let queue_id = QueueId::new(self.queues.len());
        let completion_id = CompletionRingId::new(self.queues.len());
        Ok(PreparedExactQueue {
            context: QueueContext {
                entry: QueueDirectoryEntry::new(queue_id, completion_id, config),
                frontend: io::exact(),
            },
        })
    }

    pub(in crate::runner) fn install_dma_bus_queue(
        &mut self,
        prepared: PreparedExactQueue,
    ) -> QueueConfigHandle {
        let entry = prepared.context.entry;
        self.queues.push(prepared.context);
        QueueConfigHandle::new(self.epoch, entry)
    }

    pub(in crate::runner) fn ring_dma_bus_doorbell(
        &mut self,
        queue: EpochQueue,
    ) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        let context = self.queue(queue_id)?;
        if !context.frontend.is_exact() {
            return Err(ModelError::UnknownQueue(queue_id));
        }
        if !self.runnable.contains(&queue_id) {
            self.runnable.push_back(queue_id);
        }
        Ok(())
    }

    pub(in crate::runner) fn validate_dma_bus_queue(&self, queue: EpochQueue) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        let context = self.queue(queue_id)?;
        if context.frontend.is_exact() {
            Ok(())
        } else {
            Err(ModelError::UnknownQueue(queue_id))
        }
    }

    pub(in crate::runner) fn pending_dma_result(&self) -> Option<super::super::PendingDmaResult> {
        match self.pending_retirement {
            Some(super::retirement::PendingRetirement::Completion(success)) => {
                Some(super::super::PendingDmaResult::Completion(
                    pdma_model_contract::semantic::CompletionRecord::new(
                        success.queue,
                        success.command,
                        pdma_model_contract::semantic::CompletionStatus::Success,
                    ),
                ))
            }
            Some(super::retirement::PendingRetirement::Fault { queue, .. }) => self
                .queue(queue)
                .ok()
                .and_then(|context| context.frontend.pending_exact_fault_bytes())
                .and_then(|bytes| io::exact::decode_event(&bytes))
                .map(super::super::PendingDmaResult::Event),
            None => None,
        }
    }
}
