use super::QueuePortDeviceModel;
use super::io::PublicationOutcome;
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::AsyncStallReason;
use crate::runner::backend::ModelStepStatus;
use crate::runner::bus::control::BusControlState;
use crate::{ModelError, ModelResult};
use pdma_model_contract::completion::{CompletionRecord, CompletionStatus};
use pdma_model_contract::event::MemoryFault;
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
    Fault {
        queue: QueueId,
        source: Option<ModelError>,
    },
}

pub(super) fn publish_completion(
    device: &mut QueuePortDeviceModel,
    dma_space: &PdmaMemoryMappings,
    bus_control: &mut BusControlState,
    success: SuccessRetirement,
) -> ModelResult<ModelStepStatus> {
    let completion =
        CompletionRecord::new(success.queue, success.command, CompletionStatus::Success);
    device
        .trace
        .execution_succeeded(success.queue, success.operation, success.command);
    if state::completion_publication_is_full(device, bus_control, success)? {
        state::consume_command(device, bus_control, success.queue)?;
        device.pending_retirement = Some(PendingRetirement::Completion(success));
        device
            .trace
            .stall(Some(success.queue), "CompletionPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    let publication = device
        .queue_mut(success.queue)?
        .frontend
        .publish_completion(dma_space, bus_control, success.queue, completion);
    if publication == PublicationOutcome::Retry {
        state::consume_command(device, bus_control, success.queue)?;
        device.pending_retirement = Some(PendingRetirement::Completion(success));
        device
            .trace
            .stall(Some(success.queue), "CompletionPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    device.trace.completion_published(completion);
    device.latch_irq_source(IrqSource::Completion(success.completion_ring));
    state::retire_command(device, dma_space, bus_control, success.queue)?;
    Ok(ModelStepStatus::Progressed)
}

pub(super) fn validate_success_completion_route(
    device: &mut QueuePortDeviceModel,
    dma_space: &PdmaMemoryMappings,
    bus_control: &mut BusControlState,
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
) -> ModelResult<()> {
    if let Err(source) = device.queue_by_completion(completion_ring_id).map(|_| ()) {
        state::retire_command(device, dma_space, bus_control, queue_id)?;
        return Err(source);
    }
    Ok(())
}

pub(super) fn retry_pending_retirement(
    device: &mut QueuePortDeviceModel,
    dma_space: &PdmaMemoryMappings,
    bus_control: &mut BusControlState,
) -> ModelResult<ModelStepStatus> {
    let Some(pending) = device.pending_retirement else {
        return Ok(ModelStepStatus::Idle);
    };
    match pending {
        PendingRetirement::Completion(success) => {
            retry_pending_completion(device, dma_space, bus_control, success)
        }
        PendingRetirement::Fault { queue, source } => {
            retry_pending_fault(device, dma_space, bus_control, queue, source)
        }
    }
}

fn retry_pending_completion(
    device: &mut QueuePortDeviceModel,
    dma_space: &PdmaMemoryMappings,
    bus_control: &mut BusControlState,
    pending: SuccessRetirement,
) -> ModelResult<ModelStepStatus> {
    let completion =
        CompletionRecord::new(pending.queue, pending.command, CompletionStatus::Success);
    if state::completion_publication_is_full(device, bus_control, pending)? {
        device
            .trace
            .stall(Some(pending.queue), "CompletionPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    let publication = device
        .queue_mut(pending.queue)?
        .frontend
        .publish_completion(dma_space, bus_control, pending.queue, completion);
    if publication == PublicationOutcome::Retry {
        device
            .trace
            .stall(Some(pending.queue), "CompletionPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    device.trace.completion_published(completion);
    device.latch_irq_source(IrqSource::Completion(pending.completion_ring));
    device.pending_retirement = None;
    device.requeue_if_command_pending(dma_space, bus_control, pending.queue)?;
    Ok(ModelStepStatus::Progressed)
}

fn retry_pending_fault(
    device: &mut QueuePortDeviceModel,
    dma_space: &PdmaMemoryMappings,
    bus_control: &mut BusControlState,
    queue_id: QueueId,
    source: Option<ModelError>,
) -> ModelResult<ModelStepStatus> {
    if state::fault_publication_is_full(device, bus_control, queue_id)? {
        device
            .trace
            .stall(Some(queue_id), "FaultRetirementPendingRetry");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::FaultRetirementPendingRetry,
        ));
    }
    let publication = device.publish_frontend_fault(queue_id, dma_space, bus_control)?;
    if publication == PublicationOutcome::Retry {
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::FaultRetirementPendingRetry,
        ));
    }
    device.latch_irq_source(IrqSource::Event(queue_id));
    device.pending_retirement = None;
    device.requeue_if_command_pending(dma_space, bus_control, queue_id)?;
    source.map_or(Ok(ModelStepStatus::Progressed), Err)
}

pub(super) fn publish_fault(
    device: &mut QueuePortDeviceModel,
    dma_space: &PdmaMemoryMappings,
    bus_control: &mut BusControlState,
    fault: FaultRetirement,
) -> ModelResult<ModelStepStatus> {
    let event = pdma_model_contract::EventRecord::from_memory_fault(
        fault.queue_id,
        fault.command_id,
        fault.fault,
    );
    device.trace.execution_faulted(event);
    let source = device
        .queue_mut(fault.queue_id)?
        .frontend
        .stage_execution_fault(fault)?;
    if state::fault_publication_is_full(device, bus_control, fault.queue_id)? {
        state::consume_command(device, bus_control, fault.queue_id)?;
        device.pending_retirement = Some(PendingRetirement::Fault {
            queue: fault.queue_id,
            source: Some(source),
        });
        device
            .trace
            .stall(Some(fault.queue_id), "EventPublicationPressure");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::EventPublicationPressure,
        ));
    }
    let publication = device.publish_frontend_fault(fault.queue_id, dma_space, bus_control)?;
    if publication == PublicationOutcome::Retry {
        state::consume_command(device, bus_control, fault.queue_id)?;
        device.pending_retirement = Some(PendingRetirement::Fault {
            queue: fault.queue_id,
            source: Some(source),
        });
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::EventPublicationPressure,
        ));
    }
    device.latch_irq_source(IrqSource::Event(fault.queue_id));
    state::retire_command(device, dma_space, bus_control, fault.queue_id)?;
    Err(ModelError::from_memory_fault(
        fault.fault.buffer(),
        fault.fault.error_kind(),
    ))
}

pub(super) fn cache_frontend_fault(
    device: &mut QueuePortDeviceModel,
    bus_control: &mut BusControlState,
    fault: super::io::NormalizedFault,
) -> ModelResult<ModelStepStatus> {
    state::consume_command(device, bus_control, fault.queue)?;
    device.pending_retirement = Some(PendingRetirement::Fault {
        queue: fault.queue,
        source: None,
    });
    Ok(ModelStepStatus::Progressed)
}

mod state;
