use super::AsyncStallReason;
use super::bus::control::BusControlState;
use super::native::QueuePortDeviceModel;
use super::native::exact_api::PreparedExactQueue;
use crate::dma_space::PdmaMemoryMappings;
use crate::{ModelError, ModelResult};
use pdma_model_contract::completion::CompletionRecord;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::event::EventRecord;
use pdma_model_contract::irq::{IrqNotification, IrqRoute, IrqSnapshot};
use pdma_model_contract::queue::{QueueConfig, QueueConfigHandle};
use pdma_model_contract::trace::TraceEvent;
use pdma_model_contract::{DeviceEpoch, EpochIrqSource, ResourceConfig};

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
    pub(super) fn queue_port_with_resource_config(config: ResourceConfig) -> Self {
        if config == ResourceConfig::default() {
            Self::QueuePort(QueuePortDeviceModel::new())
        } else {
            Self::QueuePort(QueuePortDeviceModel::with_resource_config(config))
        }
    }

    pub(super) fn step(
        &mut self,
        dma_space: &mut PdmaMemoryMappings,
        bus_control: &mut BusControlState,
    ) -> ModelResult<ModelStepStatus> {
        match self {
            Self::QueuePort(device) => device.step(dma_space, bus_control),
        }
    }

    pub(super) fn has_work(
        &self,
        dma_space: &PdmaMemoryMappings,
        bus_control: &BusControlState,
    ) -> bool {
        match self {
            Self::QueuePort(device) => device.has_work(dma_space, bus_control),
        }
    }

    pub(super) fn create_queue(&mut self, config: QueueConfig) -> ModelResult<QueueConfigHandle> {
        match self {
            Self::QueuePort(device) => device.create_queue(config),
        }
    }

    pub(super) fn create_dma_bus_queue(
        &mut self,
        config: QueueConfig,
    ) -> ModelResult<QueueConfigHandle> {
        match self {
            Self::QueuePort(device) => device.create_dma_bus_queue(config),
        }
    }

    pub(super) fn submit_command(
        &mut self,
        queue: QueueConfigHandle,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        match self {
            Self::QueuePort(device) => device.submit_command(queue, command),
        }
    }

    pub(super) fn ring_doorbell(&mut self, queue: QueueConfigHandle) -> ModelResult<()> {
        self.validate_epoch(queue.epoch())?;
        match self {
            Self::QueuePort(device) => device.ring_doorbell(queue),
        }
    }

    pub(super) fn poll_completion(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<CompletionRecord>> {
        self.validate_epoch(queue.epoch())?;
        match self {
            Self::QueuePort(device) => device.poll_completion(queue),
        }
    }

    pub(super) fn poll_queue_event(
        &mut self,
        queue: QueueConfigHandle,
    ) -> ModelResult<Option<EventRecord>> {
        self.validate_epoch(queue.epoch())?;
        match self {
            Self::QueuePort(device) => device.poll_queue_event(queue),
        }
    }

    pub(super) fn enable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        match self {
            Self::QueuePort(device) => device.enable_irq_source(source),
        }
    }

    pub(super) fn configure_irq_route(
        &mut self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        match self {
            Self::QueuePort(device) => device.configure_irq_route(source, route),
        }
    }

    pub(super) fn disable_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        match self {
            Self::QueuePort(device) => device.disable_irq_source(source),
        }
    }

    pub(super) fn mask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        match self {
            Self::QueuePort(device) => device.mask_irq_source(source),
        }
    }

    pub(super) fn unmask_irq_source(&mut self, source: EpochIrqSource) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        match self {
            Self::QueuePort(device) => device.unmask_irq_source(source),
        }
    }

    pub(super) fn ack_irq_source(
        &mut self,
        bus_control: &BusControlState,
        source: EpochIrqSource,
    ) -> ModelResult<()> {
        self.validate_epoch(source.epoch())?;
        match self {
            Self::QueuePort(device) => device.ack_irq_source(bus_control, source),
        }
    }

    pub(super) fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        match self {
            Self::QueuePort(device) => device.irq_snapshot(),
        }
    }

    pub(super) fn take_irq_notification(&mut self) -> Option<IrqNotification> {
        match self {
            Self::QueuePort(device) => device.take_irq_notification(),
        }
    }

    pub(super) const fn epoch(&self) -> DeviceEpoch {
        match self {
            Self::QueuePort(device) => device.epoch(),
        }
    }

    pub(super) fn reset(&mut self, epoch: DeviceEpoch) {
        match self {
            Self::QueuePort(device) => device.reset(epoch),
        }
    }

    pub(in crate::runner) fn ring_dma_bus_doorbell(
        &mut self,
        queue: pdma_model_contract::semantic::EpochQueue,
    ) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.ring_dma_bus_doorbell(queue),
        }
    }

    pub(in crate::runner) fn prepare_dma_bus_queue(
        &mut self,
        config: QueueConfig,
    ) -> ModelResult<PreparedExactQueue> {
        match self {
            Self::QueuePort(device) => device.prepare_dma_bus_queue(config),
        }
    }

    pub(in crate::runner) fn install_dma_bus_queue(
        &mut self,
        prepared: PreparedExactQueue,
    ) -> QueueConfigHandle {
        match self {
            Self::QueuePort(device) => device.install_dma_bus_queue(prepared),
        }
    }

    pub(in crate::runner) fn validate_dma_bus_queue(
        &self,
        queue: pdma_model_contract::semantic::EpochQueue,
    ) -> ModelResult<()> {
        match self {
            Self::QueuePort(device) => device.validate_dma_bus_queue(queue),
        }
    }

    pub(in crate::runner) fn next_queue_id(&self) -> pdma_model_contract::QueueId {
        match self {
            Self::QueuePort(device) => device.next_queue_id(),
        }
    }

    pub(in crate::runner) fn pending_dma_result(&self) -> Option<super::PendingDmaResult> {
        match self {
            Self::QueuePort(device) => device.pending_dma_result(),
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

    pub(super) const fn native_steps(&self) -> usize {
        match self {
            Self::QueuePort(device) => device.native_steps(),
        }
    }

    pub(super) fn trace_json(&self) -> String {
        match self {
            Self::QueuePort(device) => device.trace_json(),
        }
    }

    fn validate_epoch(&self, actual: DeviceEpoch) -> ModelResult<()> {
        let expected = self.epoch();
        if actual == expected {
            Ok(())
        } else {
            Err(ModelError::StaleDeviceEpoch { expected, actual })
        }
    }
}
