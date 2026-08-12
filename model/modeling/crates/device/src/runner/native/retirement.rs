mod dma;

use super::QueuePortDeviceModel;
use crate::dma_space::DmaSpace;
use crate::runner::AsyncStallReason;
use crate::runner::PendingDmaResult;
use crate::runner::backend::ModelStepStatus;
use crate::runner::bus::control::BusControlState;
use crate::{ModelError, ModelResult};
use pdma_model_contract::completion::{CompletionRecord, CompletionStatus};
use pdma_model_contract::event::{EventRecord, MemoryFault};
use pdma_model_contract::ids::{CommandId, CompletionRingId, QueueId};
use pdma_model_contract::irq::IrqSource;
use pdma_model_contract::trace::TraceOperation;

#[derive(Debug, Clone, Copy)]
pub(super) struct FaultRetirement {
    pub(super) queue_id: QueueId,
    pub(super) command_id: CommandId,
    pub(super) fault: MemoryFault,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SuccessRetirement {
    pub(super) queue: QueueId,
    pub(super) command: CommandId,
    pub(super) completion_ring: CompletionRingId,
    pub(super) operation: TraceOperation,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PendingRetirement {
    Completion(SuccessRetirement),
    Event {
        queue_id: QueueId,
        record: EventRecord,
        source: ModelError,
    },
    DmaEvent(pdma_model_contract::bridge::AbiEvent),
}

impl PendingRetirement {
    pub(super) const fn dma_result(self) -> Option<PendingDmaResult> {
        match self {
            Self::Completion(success) => Some(PendingDmaResult::Completion(CompletionRecord::new(
                success.queue,
                success.command,
                CompletionStatus::Success,
            ))),
            Self::DmaEvent(event) => Some(PendingDmaResult::Event(event)),
            Self::Event { .. } => None,
        }
    }
}

pub(super) fn publish_completion(
    device: &mut QueuePortDeviceModel,
    dma_space: &mut DmaSpace,
    bus_control: &mut BusControlState,
    success: SuccessRetirement,
) -> ModelResult<ModelStepStatus> {
    let completion =
        CompletionRecord::new(success.queue, success.command, CompletionStatus::Success);
    device
        .trace
        .execution_succeeded(success.queue, success.operation, success.command);
    if let Some(result) =
        dma::publish_completion(device, dma_space, bus_control, success, completion)
    {
        return result;
    }
    if completion_publication_is_full(device, success.completion_ring)? {
        consume_command(device, dma_space, bus_control, success.queue)?;
        device.pending_retirement = Some(PendingRetirement::Completion(success));
        device
            .trace
            .stall(Some(success.queue), "CompletionPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    device
        .completion_mut(success.completion_ring)?
        .push(completion)?;
    device.trace.completion_published(completion);
    device.latch_irq_source(IrqSource::Completion(success.completion_ring));
    retire_command(device, dma_space, bus_control, success.queue)?;
    Ok(ModelStepStatus::Progressed)
}

pub(super) fn validate_success_completion_route(
    device: &mut QueuePortDeviceModel,
    dma_space: &DmaSpace,
    bus_control: &mut BusControlState,
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
) -> ModelResult<()> {
    if let Err(source) = device.completion(completion_ring_id).map(|_| ()) {
        retire_command(device, dma_space, bus_control, queue_id)?;
        return Err(source);
    }
    Ok(())
}

pub(super) fn retry_pending_retirement(
    device: &mut QueuePortDeviceModel,
    dma_space: &mut DmaSpace,
    bus_control: &mut BusControlState,
) -> ModelResult<ModelStepStatus> {
    let Some(pending) = device.pending_retirement else {
        return Ok(ModelStepStatus::Idle);
    };
    match pending {
        PendingRetirement::Completion(success) => {
            retry_pending_completion(device, dma_space, bus_control, success)
        }
        PendingRetirement::Event {
            queue_id,
            record,
            source,
        } => retry_pending_event(device, dma_space, bus_control, queue_id, record, source),
        PendingRetirement::DmaEvent(event) => {
            dma::retry_event(device, dma_space, bus_control, event)
        }
    }
}

fn retry_pending_completion(
    device: &mut QueuePortDeviceModel,
    dma_space: &mut DmaSpace,
    bus_control: &mut BusControlState,
    pending: SuccessRetirement,
) -> ModelResult<ModelStepStatus> {
    let completion =
        CompletionRecord::new(pending.queue, pending.command, CompletionStatus::Success);
    if let Some(result) = dma::retry_completion(device, dma_space, bus_control, pending, completion)
    {
        return result;
    }
    if completion_publication_is_full(device, pending.completion_ring)? {
        device
            .trace
            .stall(Some(pending.queue), "CompletionPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    device
        .completion_mut(pending.completion_ring)?
        .push(completion)?;
    device.trace.completion_published(completion);
    device.latch_irq_source(IrqSource::Completion(pending.completion_ring));
    device.pending_retirement = None;
    device.requeue_if_command_pending(dma_space, bus_control, pending.queue)?;
    Ok(ModelStepStatus::Progressed)
}

fn retry_pending_event(
    device: &mut QueuePortDeviceModel,
    dma_space: &DmaSpace,
    bus_control: &BusControlState,
    queue_id: QueueId,
    record: EventRecord,
    source: ModelError,
) -> ModelResult<ModelStepStatus> {
    if event_publication_is_full(device, queue_id)? {
        device
            .trace
            .stall(Some(queue_id), "FaultRetirementPendingRetry");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::FaultRetirementPendingRetry,
        ));
    }
    device.events.push_back(record);
    device.trace.event_published(record);
    device.latch_irq_source(IrqSource::Event(queue_id));
    device.pending_retirement = None;
    device.requeue_if_command_pending(dma_space, bus_control, queue_id)?;
    Err(source)
}

pub(super) fn publish_fault(
    device: &mut QueuePortDeviceModel,
    dma_space: &DmaSpace,
    bus_control: &mut BusControlState,
    fault: FaultRetirement,
) -> ModelResult<ModelStepStatus> {
    let event = EventRecord::from_memory_fault(fault.queue_id, fault.command_id, fault.fault);
    device.trace.execution_faulted(event);
    if event_publication_is_full(device, fault.queue_id)? {
        consume_command(device, dma_space, bus_control, fault.queue_id)?;
        device.pending_retirement = Some(PendingRetirement::Event {
            queue_id: fault.queue_id,
            record: event,
            source: ModelError::from_memory_fault(fault.fault.buffer(), fault.fault.error_kind()),
        });
        device
            .trace
            .stall(Some(fault.queue_id), "EventPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::EventPublicationPressure,
        ));
    }
    device.events.push_back(event);
    device.trace.event_published(event);
    device.latch_irq_source(IrqSource::Event(fault.queue_id));
    retire_command(device, dma_space, bus_control, fault.queue_id)?;
    Err(ModelError::from_memory_fault(
        fault.fault.buffer(),
        fault.fault.error_kind(),
    ))
}

fn completion_publication_is_full(
    device: &QueuePortDeviceModel,
    completion_ring_id: CompletionRingId,
) -> ModelResult<bool> {
    let completion = device.completion(completion_ring_id)?;
    Ok(completion.is_full()
        || completion.len() >= device.resource_config.completion_publication_slots())
}

fn event_publication_is_full(
    device: &QueuePortDeviceModel,
    queue_id: QueueId,
) -> ModelResult<bool> {
    device.queue(queue_id)?;
    Ok(device.events.len() >= device.resource_config.event_publication_slots())
}

fn retire_command(
    device: &mut QueuePortDeviceModel,
    dma_space: &DmaSpace,
    bus_control: &mut BusControlState,
    queue_id: QueueId,
) -> ModelResult<()> {
    consume_command(device, dma_space, bus_control, queue_id)?;
    device.requeue_if_command_pending(dma_space, bus_control, queue_id)
}

fn consume_command(
    device: &mut QueuePortDeviceModel,
    dma_space: &DmaSpace,
    bus_control: &mut BusControlState,
    queue_id: QueueId,
) -> ModelResult<()> {
    device
        .queue_mut(queue_id)?
        .io
        .consume(dma_space, bus_control, queue_id)
}

pub(super) fn cache_dma_fault(
    device: &mut QueuePortDeviceModel,
    dma_space: &DmaSpace,
    bus_control: &mut BusControlState,
    event: pdma_model_contract::bridge::AbiEvent,
) -> ModelResult<ModelStepStatus> {
    consume_command(device, dma_space, bus_control, event.queue_id())?;
    device.pending_retirement = Some(PendingRetirement::DmaEvent(event));
    Ok(ModelStepStatus::Progressed)
}
