mod broadcast;
mod context;
mod copy;
pub mod exact_api;
#[doc(hidden)]
pub mod io;
mod irq;
pub mod irq_api;
mod probe;
mod reset;
mod resources;
mod retirement;
mod ring;
pub mod step;
pub mod trace;
mod trace_api;
mod vector_add;

use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use crate::engine::step::ModelStepStatus;
use crate::engine::trace::TraceSink;
use crate::hardware::execution::ExecutionUnit;
use crate::hardware::queue::PdmaQueue;
use crate::hardware::retirement::RetirementUnit;
use crate::hardware::scheduler::Scheduler;
use crate::{ModelError, ModelResult};
use irq::PdmaIrqController;
use pdma_model_contract::semantic::{
    Command, CompletionRecord, CompletionRingId, DeviceEpoch, EventRecord, IrqLine, IrqVector,
    QueueConfig, QueueConfigHandle, QueueId, ResourceConfig,
};

use context::NativeExecution;

#[derive(Debug)]
pub(crate) struct DeviceExecution {
    epoch: DeviceEpoch,
    queues: Vec<PdmaQueue>,
    scheduler: Scheduler,
    retirement: RetirementUnit,
    irq: PdmaIrqController,
    execution: ExecutionUnit,
    trace: TraceSink,
}

impl DeviceExecution {
    pub(super) fn with_resource_config(resource_config: ResourceConfig) -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            queues: Vec::new(),
            scheduler: Scheduler::default(),
            retirement: RetirementUnit::default(),
            irq: PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0))),
            execution: ExecutionUnit::new(resource_config),
            trace: TraceSink::default(),
        }
    }

    pub(super) const fn resource_config(&self) -> ResourceConfig {
        self.execution.resources()
    }

    pub(super) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        let queue_id = QueueId::new(self.queues.len());
        let completion_id = CompletionRingId::new(self.queues.len());
        let queue = PdmaQueue::semantic(queue_id, completion_id, config)?;
        let entry = queue.entry;
        self.queues.push(queue);
        Ok(QueueConfigHandle::new(self.epoch, entry))
    }

    pub(super) fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: Command,
    ) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        self.queue_mut(queue_id)?
            .io
            .submit_semantic(queue_id, command)
            .map(|(submitted_command, operation)| {
                self.trace
                    .command_submitted(queue_id, operation, submitted_command);
            })
    }

    pub(super) fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        self.queue(queue_id)?;
        self.scheduler.enqueue(queue_id);
        self.trace.doorbell_runnable(queue_id);
        Ok(())
    }

    pub(super) fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.validate_epoch(queue.epoch())?;
        let completion_ring_id = queue.completion_ring_id();
        let completion = self
            .queue_by_completion_mut(completion_ring_id)?
            .io
            .poll_semantic_completion();
        if let Some(record) = completion {
            self.trace.completion_polled(record);
        }
        Ok(completion)
    }

    pub(super) fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        let event = self.queue_mut(queue_id)?.io.poll_semantic_event(queue_id);
        if event.is_none() {
            self.trace.event_poll(queue_id, None);
            return Ok(None);
        }
        self.trace.event_poll(queue_id, event);
        Ok(event)
    }

    pub(super) fn step(
        &mut self,
        dma_space: &mut PdmaMemoryMappings,
        queue_control: &mut QueueControlState,
    ) -> ModelResult<ModelStepStatus> {
        step::step(NativeExecution {
            device: self,
            dma_space,
            queue_control,
        })
    }

    pub(super) fn has_work(
        &self,
        _dma_space: &PdmaMemoryMappings,
        queue_control: &QueueControlState,
    ) -> bool {
        self.retirement.is_pending()
            || !self.scheduler.is_idle()
            || self
                .queues
                .iter()
                .any(|queue| queue.io.has_pending(queue_control, queue.entry.queue_id()))
    }

    pub(super) const fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }

    pub(crate) fn next_queue_id(&self) -> QueueId {
        QueueId::new(self.queues.len())
    }

    fn validate_epoch(&self, actual: DeviceEpoch) -> ModelResult<()> {
        if actual == self.epoch {
            Ok(())
        } else {
            Err(ModelError::StaleDeviceEpoch {
                expected: self.epoch,
                actual,
            })
        }
    }

    fn queue(&self, id: QueueId) -> ModelResult<&PdmaQueue> {
        self.queues
            .iter()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    fn queue_mut(&mut self, id: QueueId) -> ModelResult<&mut PdmaQueue> {
        self.queues
            .iter_mut()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    fn queue_by_completion(&self, id: CompletionRingId) -> ModelResult<&PdmaQueue> {
        self.queues
            .iter()
            .find(|queue| queue.entry.completion_ring_id() == id)
            .ok_or(ModelError::UnknownCompletionRing(id))
    }

    fn queue_by_completion_mut(&mut self, id: CompletionRingId) -> ModelResult<&mut PdmaQueue> {
        self.queues
            .iter_mut()
            .find(|queue| queue.entry.completion_ring_id() == id)
            .ok_or(ModelError::UnknownCompletionRing(id))
    }

    fn publish_frontend_fault(
        &mut self,
        queue: QueueId,
        memory: &PdmaMemoryMappings,
        control: &mut QueueControlState,
    ) -> ModelResult<io::PublicationOutcome> {
        let context = self
            .queues
            .iter_mut()
            .find(|context| context.entry.queue_id() == queue)
            .ok_or(ModelError::UnknownQueue(queue))?;
        Ok(context.io.publish_fault(memory, control, &mut self.trace))
    }

    fn requeue_if_command_pending(
        &mut self,
        _dma_space: &PdmaMemoryMappings,
        queue_control: &QueueControlState,
        queue_id: QueueId,
    ) -> ModelResult<()> {
        if self
            .queue(queue_id)?
            .io
            .has_pending(queue_control, queue_id)
        {
            self.scheduler.enqueue(queue_id);
        }
        Ok(())
    }

    const fn memory_resolver(
        queue_id: QueueId,
        command_id: pdma_model_contract::semantic::CommandId,
    ) -> io::QueueMemoryResolver {
        io::QueueMemoryResolver::new(queue_id, command_id)
    }
}
