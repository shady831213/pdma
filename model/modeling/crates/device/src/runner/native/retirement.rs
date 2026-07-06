use super::NativeDevice;
use crate::runner::AsyncStallReason;
use crate::runner::backend::BackendStepStatus;
use pdma_model_contract::completion::{CompletionRecord, CompletionStatus};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::event::{EventErrorKind, EventRecord};
use pdma_model_contract::ids::{BufferId, CommandId, CompletionRingId, QueueId};
use pdma_model_contract::trace::TraceOperation;

#[derive(Debug, Clone, Copy)]
pub(super) struct FaultRetirement {
    pub(super) queue_id: QueueId,
    pub(super) command_id: CommandId,
    pub(super) buffer: BufferId,
    pub(super) source: ModelError,
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
}

pub(super) fn publish_completion(
    device: &mut NativeDevice,
    success: SuccessRetirement,
) -> ModelResult<BackendStepStatus> {
    let completion =
        CompletionRecord::new(success.queue, success.command, CompletionStatus::Success);
    device
        .trace
        .execution_succeeded(success.queue, success.operation, success.command);
    if completion_publication_is_full(device, success.completion_ring)? {
        retire_command_without_requeue(device, success.queue)?;
        device.pending_retirement = Some(PendingRetirement::Completion(success));
        device
            .trace
            .stall(Some(success.queue), "CompletionPublicationPressure");
        return Ok(BackendStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    device
        .completion_mut(success.completion_ring)?
        .push(completion)?;
    device.trace.completion_published(completion);
    retire_command(device, success.queue)?;
    Ok(BackendStepStatus::Progressed)
}

pub(super) fn validate_success_completion_route(
    device: &mut NativeDevice,
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
) -> ModelResult<()> {
    if let Err(source) = device.completion(completion_ring_id).map(|_| ()) {
        retire_command(device, queue_id)?;
        return Err(source);
    }
    Ok(())
}

pub(super) fn retry_pending_retirement(
    device: &mut NativeDevice,
) -> ModelResult<BackendStepStatus> {
    let Some(pending) = device.pending_retirement else {
        return Ok(BackendStepStatus::Idle);
    };
    match pending {
        PendingRetirement::Completion(success) => retry_pending_completion(device, success),
        PendingRetirement::Event {
            queue_id,
            record,
            source,
        } => retry_pending_event(device, queue_id, record, source),
    }
}

fn retry_pending_completion(
    device: &mut NativeDevice,
    pending: SuccessRetirement,
) -> ModelResult<BackendStepStatus> {
    let completion =
        CompletionRecord::new(pending.queue, pending.command, CompletionStatus::Success);
    if completion_publication_is_full(device, pending.completion_ring)? {
        device
            .trace
            .stall(Some(pending.queue), "CompletionPublicationPressure");
        return Ok(BackendStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ));
    }
    device
        .completion_mut(pending.completion_ring)?
        .push(completion)?;
    device.trace.completion_published(completion);
    device.pending_retirement = None;
    device.requeue_if_command_pending(pending.queue)?;
    Ok(BackendStepStatus::Progressed)
}

fn retry_pending_event(
    device: &mut NativeDevice,
    queue_id: QueueId,
    record: EventRecord,
    source: ModelError,
) -> ModelResult<BackendStepStatus> {
    if event_publication_is_full(device) {
        device
            .trace
            .stall(Some(queue_id), "FaultRetirementPendingRetry");
        return Ok(BackendStepStatus::Stalled(
            AsyncStallReason::FaultRetirementPendingRetry,
        ));
    }
    device.events.push_back(record);
    device.trace.event_published(record);
    device.pending_retirement = None;
    device.requeue_if_command_pending(queue_id)?;
    Err(source)
}

pub(super) fn publish_fault(
    device: &mut NativeDevice,
    fault: FaultRetirement,
) -> ModelResult<BackendStepStatus> {
    let event = EventRecord::new(
        fault.queue_id,
        fault.command_id,
        event_error_kind(fault.source)?,
        fault.buffer,
    );
    device.trace.execution_faulted(event);
    if event_publication_is_full(device) {
        retire_command_without_requeue(device, fault.queue_id)?;
        device.pending_retirement = Some(PendingRetirement::Event {
            queue_id: fault.queue_id,
            record: event,
            source: fault.source,
        });
        device
            .trace
            .stall(Some(fault.queue_id), "EventPublicationPressure");
        return Ok(BackendStepStatus::Stalled(
            AsyncStallReason::EventPublicationPressure,
        ));
    }
    device.events.push_back(event);
    device.trace.event_published(event);
    retire_command(device, fault.queue_id)?;
    Err(fault.source)
}

fn completion_publication_is_full(
    device: &NativeDevice,
    completion_ring_id: CompletionRingId,
) -> ModelResult<bool> {
    let completion = device.completion(completion_ring_id)?;
    Ok(completion.is_full()
        || completion.len() >= device.resource_config.completion_publication_slots())
}

fn event_publication_is_full(device: &NativeDevice) -> bool {
    device.events.len() >= device.resource_config.event_publication_slots()
}

fn retire_command(device: &mut NativeDevice, queue_id: QueueId) -> ModelResult<()> {
    retire_command_without_requeue(device, queue_id)?;
    device.requeue_if_command_pending(queue_id)
}

fn retire_command_without_requeue(device: &mut NativeDevice, queue_id: QueueId) -> ModelResult<()> {
    let queue = device.queue_mut(queue_id)?;
    let _retired = queue.commands.pop();
    Ok(())
}

const fn event_error_kind(error: ModelError) -> ModelResult<EventErrorKind> {
    match error {
        ModelError::UnknownBuffer(_) => Ok(EventErrorKind::UnknownBuffer),
        ModelError::RangeOverflow => Ok(EventErrorKind::RangeOverflow),
        ModelError::OutOfBounds { len, end, .. } => Ok(EventErrorKind::OutOfBounds { len, end }),
        _ => Err(error),
    }
}
