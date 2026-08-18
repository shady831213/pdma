use super::QueuePortDeviceModel;
use super::io::NormalizedFetch;
use super::resources::ResourceDemand;
use super::retirement::PendingRetirement;
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::AsyncStallReason;
use crate::runner::bus::control::BusControlState;
use pdma_model_contract::semantic::QueueId;

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
    dma_space: &PdmaMemoryMappings,
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
    let fetched = context
        .frontend
        .probe(dma_space, bus_control, context.entry);
    match fetched {
        Ok(Some(NormalizedFetch::Command(command)))
            if !ResourceDemand::for_command(&command).fits(device.resource_config) =>
        {
            RunStateProbe::Blocked {
                queue,
                reason: AsyncStallReason::ExecutionResourcePressure,
            }
        }
        Ok(Some(NormalizedFetch::Command(_) | NormalizedFetch::Fault(_)) | None) | Err(_) => {
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
            completion_is_blocked(device, bus_control, success.queue),
        ),
        PendingRetirement::Fault { queue, .. } => (
            queue,
            AsyncStallReason::FaultRetirementPendingRetry,
            device.queue(queue).map_or(true, |context| {
                context.frontend.fault_blocked(
                    bus_control,
                    queue,
                    device.resource_config.event_publication_slots(),
                )
            }),
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
) -> bool {
    device.queue(queue).is_ok_and(|context| {
        context.frontend.completion_blocked(
            bus_control,
            queue,
            device.resource_config.completion_publication_slots(),
        )
    })
}
