use super::ring::NativeRing;
use super::{QueueContext, QueuePortDeviceModel, io, retirement};
use pdma_model_contract::abi::EpochQueue;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::{
    CompletionRingId, QueueConfig, QueueConfigHandle, QueueDirectoryEntry, QueueId,
};

pub(in crate::runner) struct PreparedDmaBusQueue {
    context: QueueContext,
    completion: NativeRing<CompletionRecord>,
}

impl PreparedDmaBusQueue {
    pub(in crate::runner) const fn queue_id(&self) -> QueueId {
        self.context.entry.queue_id()
    }
}

impl QueuePortDeviceModel {
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
    ) -> ModelResult<PreparedDmaBusQueue> {
        self.queues
            .try_reserve_exact(1)
            .map_err(|_| ModelError::DmaAllocationExhausted)?;
        self.completions
            .try_reserve_exact(1)
            .map_err(|_| ModelError::DmaAllocationExhausted)?;
        let completion = NativeRing::with_capacity(config.completion_depth())?;
        let queue_id = QueueId::new(self.queues.len());
        let completion_id = CompletionRingId::new(self.completions.len());
        Ok(PreparedDmaBusQueue {
            context: QueueContext {
                entry: QueueDirectoryEntry::new(queue_id, completion_id, config),
                io: io::QueueIo::dma_bus(),
            },
            completion,
        })
    }

    pub(in crate::runner) fn install_dma_bus_queue(
        &mut self,
        prepared: PreparedDmaBusQueue,
    ) -> QueueConfigHandle {
        let entry = prepared.context.entry;
        self.queues.push(prepared.context);
        self.completions.push(prepared.completion);
        QueueConfigHandle::new(self.epoch, entry)
    }

    pub(in crate::runner) fn ring_dma_bus_doorbell(
        &mut self,
        queue: EpochQueue,
    ) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        let context = self.queue(queue_id)?;
        if !matches!(context.io, io::QueueIo::DmaBus(_)) {
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
        if matches!(context.io, io::QueueIo::DmaBus(_)) {
            Ok(())
        } else {
            Err(ModelError::UnknownQueue(queue_id))
        }
    }

    pub(in crate::runner) fn pending_dma_result(&self) -> Option<super::super::PendingDmaResult> {
        self.pending_retirement
            .and_then(retirement::PendingRetirement::dma_result)
    }
}
