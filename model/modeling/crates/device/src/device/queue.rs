use super::DeviceCore;
use crate::ModelResult;
use crate::queue::exact::{ActiveQueue, PdmaControlError, PendingDmaResult, PreparedExactQueue};
use pdma_model_contract::semantic::{
    Command, CompletionRecord, EpochQueue, EventRecord, QueueConfig, QueueConfigHandle, QueueId,
};

impl DeviceCore {
    pub(crate) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        self.queues.create_semantic(self.epoch, config)
    }

    pub(crate) fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: Command,
    ) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let submitted = self.queues.submit_semantic(queue.queue_id(), command)?;
        self.trace
            .command_submitted(submitted.queue, submitted.operation, submitted.command);
        Ok(())
    }

    pub(crate) fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue = queue.queue_id();
        self.queues.entry(queue)?;
        self.scheduler.enqueue(queue);
        self.trace.doorbell_runnable(queue);
        Ok(())
    }

    pub(crate) fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.validate_epoch(queue.epoch())?;
        let completion = self
            .queues
            .poll_semantic_completion(queue.completion_ring_id())?;
        if let Some(record) = completion {
            self.trace.completion_polled(record);
        }
        Ok(completion)
    }

    pub(crate) fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.validate_epoch(queue.epoch())?;
        let queue = queue.queue_id();
        let event = self.queues.poll_semantic_event(queue)?;
        self.trace.event_poll(queue, event);
        Ok(event)
    }

    pub(crate) fn prepare_dma_bus_queue(
        &mut self,
        prepared: PreparedExactQueue,
    ) -> ModelResult<PreparedExactQueue> {
        self.queues.prepare_exact(prepared)
    }

    pub(crate) fn install_dma_bus_queue(
        &mut self,
        prepared: PreparedExactQueue,
    ) -> QueueConfigHandle {
        self.queues.install_exact(self.epoch, prepared)
    }

    pub(crate) fn ring_dma_bus_doorbell(&mut self, queue: EpochQueue) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        self.queues.validate_exact(queue.queue_id())?;
        self.scheduler.enqueue(queue.queue_id());
        Ok(())
    }

    pub(crate) fn validate_dma_bus_queue(&self, queue: EpochQueue) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        self.queues.validate_exact(queue.queue_id())
    }

    pub(crate) fn exact_ring(&self, queue: EpochQueue) -> Result<&ActiveQueue, PdmaControlError> {
        self.validate_exact_epoch(queue)?;
        self.queues.exact_ring(queue.queue_id())
    }

    pub(crate) fn exact_ring_mut(
        &mut self,
        queue: EpochQueue,
    ) -> Result<&mut ActiveQueue, PdmaControlError> {
        self.validate_exact_epoch(queue)?;
        self.queues.exact_ring_mut(queue.queue_id())
    }

    pub(crate) fn pending_dma_result(&self) -> Option<PendingDmaResult> {
        match self.retirement.pending() {
            Some(crate::retirement::PendingRetirement::Completion(success)) => {
                Some(PendingDmaResult::Completion(CompletionRecord::new(
                    success.route.queue,
                    success.route.command,
                    pdma_model_contract::semantic::CompletionStatus::Success,
                )))
            }
            Some(crate::retirement::PendingRetirement::Fault { queue, .. }) => {
                self.queues.pending_exact_fault(queue)
            }
            None => None,
        }
    }

    pub(crate) fn next_queue_id(&self) -> QueueId {
        self.queues.next_queue_id()
    }

    fn validate_exact_epoch(&self, queue: EpochQueue) -> Result<(), PdmaControlError> {
        if queue.epoch() == self.epoch {
            Ok(())
        } else {
            Err(PdmaControlError::StaleEpoch {
                expected: self.epoch,
                actual: queue.epoch(),
            })
        }
    }
}
