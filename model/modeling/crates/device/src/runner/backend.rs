use super::AsyncStallReason;
use super::native::QueuePortDeviceModel;
use pdma_model_contract::ResourceConfig;
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::ids::{BufferId, CompletionRingId, QueueId};
use pdma_model_contract::irq::{IrqSnapshot, IrqSource};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::trace::TraceEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelStepStatus {
    Progressed,
    Idle,
    Stalled(AsyncStallReason),
}

#[derive(Debug)]
pub(super) enum DeviceModel {
    QueuePort(QueuePortDeviceModel),
}

impl DeviceModel {
    pub(super) fn queue_port() -> Self {
        Self::queue_port_with_resource_config(ResourceConfig::default())
    }

    pub(super) fn queue_port_with_resource_config(config: ResourceConfig) -> Self {
        if config == ResourceConfig::default() {
            Self::QueuePort(QueuePortDeviceModel::new())
        } else {
            Self::QueuePort(QueuePortDeviceModel::with_resource_config(config))
        }
    }

    pub(super) fn step(&mut self) -> ModelResult<ModelStepStatus> {
        match self {
            Self::QueuePort(device) => device.step(),
        }
    }

    pub(super) fn has_work(&self) -> bool {
        match self {
            Self::QueuePort(device) => device.has_work(),
        }
    }

    pub(super) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        match self {
            Self::QueuePort(device) => device.create_queue(config),
        }
    }

    pub(super) fn allocate_buffer(&mut self, len: usize) -> BufferId {
        match self {
            Self::QueuePort(device) => device.allocate_buffer(len),
        }
    }

    pub(super) fn write_buffer(
        &mut self,
        id: BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.write_buffer(id, offset, data),
        }
    }

    pub(super) fn read_buffer(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        match self {
            Self::QueuePort(device) => device.read_buffer(id, offset, len),
        }
    }

    pub(super) fn submit_command(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.submit_command(queue_id, command),
        }
    }

    pub(super) fn ring_doorbell(&mut self, queue_id: QueueId) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.ring_doorbell(queue_id),
        }
    }

    pub(super) fn poll_completion(
        &mut self,
        completion_ring_id: CompletionRingId,
    ) -> ModelResult<Option<CompletionRecord>> {
        match self {
            Self::QueuePort(device) => device.poll_completion(completion_ring_id),
        }
    }

    pub(super) fn poll_queue_event(
        &mut self,
        queue_id: QueueId,
    ) -> ModelResult<Option<EventRecord>> {
        match self {
            Self::QueuePort(device) => device.poll_queue_event(queue_id),
        }
    }

    pub(super) fn enable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.enable_irq_source(source),
        }
    }

    pub(super) fn disable_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.disable_irq_source(source),
        }
    }

    pub(super) fn mask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.mask_irq_source(source),
        }
    }

    pub(super) fn unmask_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.unmask_irq_source(source),
        }
    }

    pub(super) fn ack_irq_source(&mut self, source: IrqSource) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.ack_irq_source(source),
        }
    }

    pub(super) fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        match self {
            Self::QueuePort(device) => device.irq_snapshot(),
        }
    }

    pub(super) const fn enable_tracing(&mut self) {
        match self {
            Self::QueuePort(device) => device.enable_tracing(),
        }
    }

    pub(super) const fn disable_tracing(&mut self) {
        match self {
            Self::QueuePort(device) => device.disable_tracing(),
        }
    }

    pub(super) fn clear_trace(&mut self) {
        match self {
            Self::QueuePort(device) => device.clear_trace(),
        }
    }

    pub(super) fn trace_events(&self) -> Vec<TraceEvent> {
        match self {
            Self::QueuePort(device) => device.trace_events(),
        }
    }

    pub(super) fn trace_json(&self) -> String {
        match self {
            Self::QueuePort(device) => device.trace_json(),
        }
    }
}
