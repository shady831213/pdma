mod broadcast;
mod copy;
pub(in crate::runner) mod dma_bus_api;
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

use super::backend::ModelStepStatus;
use super::bus::control::BusControlState;
use crate::dma_space::DmaSpace;
use crate::trace::{TraceSink, command_id, operation_from_external};
use irq::PdmaIrqController;
use pdma_model_contract::{
    CompletionRecord, CompletionRingId, DeviceEpoch, EventRecord, ExternalCommand, IrqLine,
    IrqVector, ModelError, ModelResult, QueueConfig, QueueConfigHandle, QueueDirectoryEntry,
    QueueId, ResourceConfig, TraceEvent,
};
use ring::NativeRing;
use std::collections::VecDeque;

#[derive(Debug)]
struct QueueContext {
    entry: QueueDirectoryEntry,
    io: io::QueueIo,
}

pub(in crate::runner) struct NativeExecution<'a> {
    pub(in crate::runner) device: &'a mut QueuePortDeviceModel,
    pub(in crate::runner) dma_space: &'a mut DmaSpace,
    pub(in crate::runner) bus_control: &'a mut BusControlState,
}

#[derive(Debug)]
pub(super) struct QueuePortDeviceModel {
    epoch: DeviceEpoch,
    queues: Vec<QueueContext>,
    completions: Vec<NativeRing<CompletionRecord>>,
    events: VecDeque<EventRecord>,
    runnable: VecDeque<QueueId>,
    pending_retirement: Option<retirement::PendingRetirement>,
    irq: PdmaIrqController,
    resource_config: ResourceConfig,
    trace: TraceSink,
}

impl QueuePortDeviceModel {
    pub(super) fn new() -> Self {
        Self::with_resource_config(ResourceConfig::default())
    }

    pub(super) fn with_resource_config(resource_config: ResourceConfig) -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            queues: Vec::new(),
            completions: Vec::new(),
            events: VecDeque::new(),
            runnable: VecDeque::new(),
            pending_retirement: None,
            irq: PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0))),
            resource_config,
            trace: TraceSink::default(),
        }
    }

    pub(super) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        let io = io::QueueIo::semantic(config.command_depth())?;
        self.create_queue_with_io(config, io)
    }

    fn create_queue_with_io(
        &mut self,
        config: QueueConfig,
        io: io::QueueIo,
    ) -> ModelResult<QueueConfigHandle> {
        let completions = NativeRing::with_capacity(config.completion_depth())?;
        let queue_id = QueueId::new(self.queues.len());
        let completion_id = CompletionRingId::new(self.completions.len());
        let entry = QueueDirectoryEntry::new(queue_id, completion_id, config);
        self.queues.push(QueueContext { entry, io });
        self.completions.push(completions);
        Ok(QueueConfigHandle::new(self.epoch, entry))
    }

    pub(super) fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        let queue_id = queue.queue_id();
        let operation = operation_from_external(&command);
        let submitted_command = command_id(&command);
        self.queue_mut(queue_id)?
            .io
            .semantic_mut()
            .ok_or(ModelError::UnknownQueue(queue_id))?
            .submit(command)
            .map(|()| {
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
            .completion_mut(completion_ring_id)
            .map(NativeRing::pop)?;
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
        self.queue(queue_id)?;
        let Some(index) = self
            .events
            .iter()
            .position(|event| event.queue_id() == queue_id)
        else {
            self.trace.event_poll(queue_id, None);
            return Ok(None);
        };
        let event = self.events.remove(index);
        self.trace.event_poll(queue_id, event);
        Ok(event)
    }

    pub(super) fn step(
        &mut self,
        dma_space: &mut DmaSpace,
        bus_control: &mut BusControlState,
    ) -> ModelResult<ModelStepStatus> {
        step::step(NativeExecution {
            device: self,
            dma_space,
            bus_control,
        })
    }

    pub(super) fn has_work(&self, dma_space: &DmaSpace, bus_control: &BusControlState) -> bool {
        self.pending_retirement.is_some()
            || !self.runnable.is_empty()
            || self.queues.iter().any(|queue| {
                queue
                    .io
                    .has_pending(dma_space, bus_control, queue.entry.queue_id())
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

    fn completion_mut(
        &mut self,
        id: CompletionRingId,
    ) -> ModelResult<&mut NativeRing<CompletionRecord>> {
        self.completions
            .get_mut(id.raw())
            .ok_or(ModelError::UnknownCompletionRing(id))
    }

    fn completion(&self, id: CompletionRingId) -> ModelResult<&NativeRing<CompletionRecord>> {
        self.completions
            .get(id.raw())
            .ok_or(ModelError::UnknownCompletionRing(id))
    }

    fn requeue_if_command_pending(
        &mut self,
        dma_space: &DmaSpace,
        bus_control: &BusControlState,
        queue_id: QueueId,
    ) -> ModelResult<()> {
        if self
            .queue(queue_id)?
            .io
            .has_pending(dma_space, bus_control, queue_id)
            && !self.runnable.contains(&queue_id)
        {
            self.runnable.push_back(queue_id);
        }
        Ok(())
    }

    fn memory_resolver(
        &self,
        queue_id: QueueId,
        command_id: pdma_model_contract::CommandId,
    ) -> ModelResult<io::QueueMemoryResolver> {
        Ok(self
            .queue(queue_id)?
            .io
            .memory_resolver(queue_id, command_id))
    }
}
