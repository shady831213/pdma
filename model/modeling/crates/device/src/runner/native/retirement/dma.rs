use super::{PendingRetirement, SuccessRetirement, consume_command, retire_command};
use crate::dma_space::DmaSpace;
use crate::runner::AsyncStallReason;
use crate::runner::backend::ModelStepStatus;
use crate::runner::bus::control::BusControlState;
use crate::runner::native::QueuePortDeviceModel;
use pdma_model_contract::abi::AbiEvent;
use pdma_model_contract::{CompletionRecord, IrqSource, ModelResult};

pub(super) fn publish_completion(
    device: &mut QueuePortDeviceModel,
    dma_space: &mut DmaSpace,
    bus_control: &mut BusControlState,
    success: SuccessRetirement,
    completion: CompletionRecord,
) -> Option<ModelResult<ModelStepStatus>> {
    let is_dma = match device.queue(success.queue) {
        Ok(queue) => queue.io.is_dma_bus(),
        Err(error) => return Some(Err(error)),
    };
    if !is_dma {
        return None;
    }
    let published = match device.queue_mut(success.queue) {
        Ok(queue) => {
            queue
                .io
                .publish_dma_completion(dma_space, bus_control, success.queue, completion)
        }
        Err(error) => return Some(Err(error)),
    };
    Some(if published {
        device.trace.completion_published(completion);
        device.latch_irq_source(IrqSource::Completion(success.completion_ring));
        retire_command(device, dma_space, bus_control, success.queue)
            .map(|()| ModelStepStatus::Progressed)
    } else {
        consume_command(device, dma_space, bus_control, success.queue).map(|()| {
            device.pending_retirement = Some(PendingRetirement::Completion(success));
            device
                .trace
                .stall(Some(success.queue), "CompletionPublicationPressure");
            ModelStepStatus::Stalled(AsyncStallReason::CompletionPublicationPressure)
        })
    })
}

pub(super) fn retry_completion(
    device: &mut QueuePortDeviceModel,
    dma_space: &mut DmaSpace,
    bus_control: &mut BusControlState,
    pending: SuccessRetirement,
    completion: CompletionRecord,
) -> Option<ModelResult<ModelStepStatus>> {
    let is_dma = match device.queue(pending.queue) {
        Ok(queue) => queue.io.is_dma_bus(),
        Err(error) => return Some(Err(error)),
    };
    if !is_dma {
        return None;
    }
    let published = match device.queue_mut(pending.queue) {
        Ok(queue) => {
            queue
                .io
                .publish_dma_completion(dma_space, bus_control, pending.queue, completion)
        }
        Err(error) => return Some(Err(error)),
    };
    Some(if published {
        device.trace.completion_published(completion);
        device.latch_irq_source(IrqSource::Completion(pending.completion_ring));
        device.pending_retirement = None;
        device
            .requeue_if_command_pending(dma_space, bus_control, pending.queue)
            .map(|()| ModelStepStatus::Progressed)
    } else {
        device
            .trace
            .stall(Some(pending.queue), "CompletionPublicationPressure");
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure,
        ))
    })
}

pub(super) fn retry_event(
    device: &mut QueuePortDeviceModel,
    dma_space: &mut DmaSpace,
    bus_control: &mut BusControlState,
    event: AbiEvent,
) -> ModelResult<ModelStepStatus> {
    if !device
        .queue_mut(event.queue_id())?
        .io
        .publish_dma_event(dma_space, bus_control, event)
    {
        device
            .trace
            .stall(Some(event.queue_id()), "FaultRetirementPendingRetry");
        return Ok(ModelStepStatus::Stalled(
            AsyncStallReason::FaultRetirementPendingRetry,
        ));
    }
    device.trace.dma_event_published(event);
    device.latch_irq_source(IrqSource::Event(event.queue_id()));
    device.pending_retirement = None;
    device.requeue_if_command_pending(dma_space, bus_control, event.queue_id())?;
    Ok(ModelStepStatus::Progressed)
}
