use super::QueuePortDeviceModel;
use super::io::QueueFetch;
use super::resources::ResourceDemand;
use super::retirement::PendingRetirement;
use crate::dma_space::DmaSpace;
use crate::runner::AsyncStallReason;
use crate::runner::bus::control::BusControlState;
use pdma_model_contract::QueueId;
use pdma_model_contract::abi::EpochQueue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runner) enum RunStateProbe {
    Idle,
    Running,
    Blocked {
        queue: QueueId,
        reason: AsyncStallReason,
    },
}

pub(in crate::runner) fn probe_run_state(
    device: &QueuePortDeviceModel,
    dma_space: &DmaSpace,
    bus_control: &BusControlState,
) -> RunStateProbe {
    if let Some(pending) = device.pending_retirement {
        return pending_probe(device, bus_control, pending);
    }
    let Some(&queue) = device.runnable.front() else {
        return if device.has_work(dma_space, bus_control) {
            RunStateProbe::Running
        } else {
            RunStateProbe::Idle
        };
    };
    let Ok(context) = device.queue(queue) else {
        return RunStateProbe::Running;
    };
    let fetched = context.io.fetch(
        dma_space,
        bus_control,
        queue,
        context.entry.completion_ring_id(),
    );
    match fetched {
        Ok(Some(QueueFetch::Command(command)))
            if !ResourceDemand::for_command(&command).fits(device.resource_config) =>
        {
            RunStateProbe::Blocked {
                queue,
                reason: AsyncStallReason::ExecutionResourcePressure,
            }
        }
        Ok(Some(QueueFetch::Command(_) | QueueFetch::Fault(_)) | None) | Err(_) => {
            RunStateProbe::Running
        }
    }
}

fn pending_probe(
    device: &QueuePortDeviceModel,
    bus_control: &BusControlState,
    pending: PendingRetirement,
) -> RunStateProbe {
    let (queue, reason, blocked) = match pending {
        PendingRetirement::Completion(success) => (
            success.queue,
            AsyncStallReason::CompletionPublicationPressure,
            completion_is_blocked(device, bus_control, success.queue, success.completion_ring),
        ),
        PendingRetirement::Event { queue_id, .. } => (
            queue_id,
            AsyncStallReason::FaultRetirementPendingRetry,
            device.events.len() >= device.resource_config.event_publication_slots(),
        ),
        PendingRetirement::DmaEvent(event) => (
            event.queue_id(),
            AsyncStallReason::FaultRetirementPendingRetry,
            dma_ring_is_full(bus_control, event.queue_id(), false),
        ),
    };
    if blocked {
        RunStateProbe::Blocked { queue, reason }
    } else {
        RunStateProbe::Running
    }
}

fn completion_is_blocked(
    device: &QueuePortDeviceModel,
    bus_control: &BusControlState,
    queue: QueueId,
    completion: pdma_model_contract::CompletionRingId,
) -> bool {
    device.queue(queue).is_ok_and(|context| {
        if context.io.is_dma_bus() {
            dma_ring_is_full(bus_control, queue, true)
        } else {
            device.completion(completion).map_or(true, |ring| {
                ring.is_full()
                    || ring.len() >= device.resource_config.completion_publication_slots()
            })
        }
    })
}

fn dma_ring_is_full(control: &BusControlState, queue: QueueId, completion: bool) -> bool {
    let queue = EpochQueue::new(control.epoch(), queue);
    control.queue(queue).map_or(true, |active| {
        if completion {
            active.completion.is_full()
        } else {
            active.event.is_full()
        }
    })
}
