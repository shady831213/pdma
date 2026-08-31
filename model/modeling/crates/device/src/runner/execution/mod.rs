mod broadcast;
mod command;
mod context;
mod copy;
pub(in crate::runner) mod exact_api;
mod io;
mod irq;
mod irq_api;
pub(in crate::runner) mod probe;
mod reset;
mod resources;
mod retirement;
mod ring;
mod step;
mod vector_add;

use crate::dma_space::PdmaMemoryMappings;
use crate::runner::adapter::bus::control::BusControlState;
use crate::runner::types::ModelStepStatus;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use irq::PdmaIrqController;
use pdma_model_contract::semantic::{
    Command, CompletionRecord, CompletionRingId, DeviceEpoch, EventRecord, IrqLine, IrqVector,
    QueueConfig, QueueConfigHandle, QueueDirectoryEntry, QueueId, ResourceConfig, TraceEvent,
};
use std::collections::VecDeque;

use context::{NativeExecution, QueueContext};

#[derive(Debug)]
pub(super) struct DeviceExecution {
    epoch: DeviceEpoch,
    queues: Vec<QueueContext>,
    runnable: VecDeque<QueueId>,
    pending_retirement: Option<retirement::PendingRetirement>,
    irq: PdmaIrqController,
    resource_config: ResourceConfig,
    trace: TraceSink,
    native_steps: usize,
}

impl DeviceExecution {
    pub(super) fn with_resource_config(resource_config: ResourceConfig) -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            queues: Vec::new(),
            runnable: VecDeque::new(),
            pending_retirement: None,
            irq: PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0))),
            resource_config,
            trace: TraceSink::default(),
            native_steps: 0,
        }
    }

    pub(super) const fn resource_config(&self) -> ResourceConfig {
        self.resource_config
    }

    pub(super) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        let frontend = io::semantic(config.command_depth(), config.completion_depth())?;
        let queue_id = QueueId::new(self.queues.len());
        let completion_id = CompletionRingId::new(self.queues.len());
        let entry = QueueDirectoryEntry::new(queue_id, completion_id, config);
        self.queues.push(QueueContext { entry, frontend });
        Ok(QueueConfigHandle::new(self.epoch, entry))
    }

    pub(super) fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: Command,
    ) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        let completion_ring = queue.completion_ring_id();
        self.queue_mut(queue_id)?
            .frontend
            .submit_semantic(queue_id, completion_ring, command)
            .map(|(submitted_command, operation)| {
                self.trace
                    .command_submitted(queue_id, operation, submitted_command);
            })
    }

    pub(super) fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        self.queue(queue_id)?;
        if !self.runnable.contains(&queue_id) {
            self.runnable.push_back(queue_id);
        }
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
            .frontend
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
        let event = self
            .queue_mut(queue_id)?
            .frontend
            .poll_semantic_event(queue_id);
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
        bus_control: &mut BusControlState,
    ) -> ModelResult<ModelStepStatus> {
        step::step(NativeExecution {
            device: self,
            dma_space,
            bus_control,
        })
    }

    pub(super) fn has_work(
        &self,
        _dma_space: &PdmaMemoryMappings,
        bus_control: &BusControlState,
    ) -> bool {
        self.pending_retirement.is_some()
            || !self.runnable.is_empty()
            || self.queues.iter().any(|queue| {
                queue
                    .frontend
                    .has_pending(bus_control, queue.entry.queue_id())
            })
    }

    pub(super) const fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }

    pub(in crate::runner) fn next_queue_id(&self) -> QueueId {
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

    pub(super) const fn enable_tracing(&mut self) {
        self.trace.enable();
    }

    pub(super) const fn disable_tracing(&mut self) {
        self.trace.disable();
    }

    pub(super) fn clear_trace(&mut self) {
        self.trace.clear();
    }

    pub(super) fn trace_events(&self) -> Vec<TraceEvent> {
        self.trace.events().to_vec()
    }

    pub(super) fn trace_json(&self) -> String {
        self.trace.to_json()
    }

    pub(super) const fn native_steps(&self) -> usize {
        self.native_steps
    }

    fn queue(&self, id: QueueId) -> ModelResult<&QueueContext> {
        self.queues
            .iter()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    fn queue_mut(&mut self, id: QueueId) -> ModelResult<&mut QueueContext> {
        self.queues
            .iter_mut()
            .find(|queue| queue.entry.queue_id() == id)
            .ok_or(ModelError::UnknownQueue(id))
    }

    fn queue_by_completion(&self, id: CompletionRingId) -> ModelResult<&QueueContext> {
        self.queues
            .iter()
            .find(|queue| queue.entry.completion_ring_id() == id)
            .ok_or(ModelError::UnknownCompletionRing(id))
    }

    fn queue_by_completion_mut(&mut self, id: CompletionRingId) -> ModelResult<&mut QueueContext> {
        self.queues
            .iter_mut()
            .find(|queue| queue.entry.completion_ring_id() == id)
            .ok_or(ModelError::UnknownCompletionRing(id))
    }

    fn publish_frontend_fault(
        &mut self,
        queue: QueueId,
        memory: &PdmaMemoryMappings,
        control: &mut BusControlState,
    ) -> ModelResult<io::PublicationOutcome> {
        let context = self
            .queues
            .iter_mut()
            .find(|context| context.entry.queue_id() == queue)
            .ok_or(ModelError::UnknownQueue(queue))?;
        Ok(context
            .frontend
            .publish_fault(memory, control, &mut self.trace))
    }

    fn requeue_if_command_pending(
        &mut self,
        _dma_space: &PdmaMemoryMappings,
        bus_control: &BusControlState,
        queue_id: QueueId,
    ) -> ModelResult<()> {
        if self
            .queue(queue_id)?
            .frontend
            .has_pending(bus_control, queue_id)
            && !self.runnable.contains(&queue_id)
        {
            self.runnable.push_back(queue_id);
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
