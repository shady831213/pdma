use super::DeviceExecution;
use super::io::NormalizedFetch;
use super::resources::ResourceDemand;
use crate::abi::dma::PdmaMemoryMappings;
use crate::abi::ring::control::QueueControlState;
use crate::hardware::retirement::PendingRetirement;
use crate::runtime::AsyncStallReason;
use pdma_model_contract::semantic::DeviceRunState;
use pdma_model_contract::semantic::QueueId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RunStateProbe {
    Idle,
    Running,
    Blocked {
        queue: QueueId,
        reason: AsyncStallReason,
    },
}

pub(super) fn probe_run_state(
    device: &DeviceExecution,
    dma_space: &PdmaMemoryMappings,
    queue_control: &QueueControlState,
) -> RunStateProbe {
    if let Some(pending) = device.retirement.pending() {
        return pending_probe(device, queue_control, pending);
    }
    let Some(queue) = device.scheduler.current() else {
        return if device.has_work(dma_space, queue_control) {
            RunStateProbe::Running
        } else {
            RunStateProbe::Idle
        };
    };
    let Ok(context) = device.queue(queue) else {
        return RunStateProbe::Running;
    };
    let fetched = context.io.probe(dma_space, queue_control, context.entry);
    match fetched {
        Ok(Some(NormalizedFetch::Command { command, .. }))
            if !ResourceDemand::for_command(&command).fits(device.execution.resources()) =>
        {
            RunStateProbe::Blocked {
                queue,
                reason: AsyncStallReason::ExecutionResourcePressure,
            }
        }
        Ok(Some(NormalizedFetch::Command { .. } | NormalizedFetch::Fault(_)) | None) | Err(_) => {
            RunStateProbe::Running
        }
    }
}

impl DeviceExecution {
    pub(crate) fn run_state(
        &self,
        dma_space: &PdmaMemoryMappings,
        queue_control: &QueueControlState,
    ) -> DeviceRunState {
        match probe_run_state(self, dma_space, queue_control) {
            RunStateProbe::Idle => DeviceRunState::Idle,
            RunStateProbe::Running => DeviceRunState::Running,
            RunStateProbe::Blocked { .. } => DeviceRunState::Blocked,
        }
    }
}

fn pending_probe(
    device: &DeviceExecution,
    queue_control: &QueueControlState,
    pending: PendingRetirement,
) -> RunStateProbe {
    let (queue, reason, blocked) = match pending {
        PendingRetirement::Completion(success) => (
            success.queue,
            AsyncStallReason::CompletionPublicationPressure,
            completion_is_blocked(device, queue_control, success.queue),
        ),
        PendingRetirement::Fault { queue, .. } => (
            queue,
            AsyncStallReason::FaultRetirementPendingRetry,
            device.queue(queue).map_or(true, |context| {
                context.io.fault_blocked(
                    queue_control,
                    queue,
                    device.execution.resources().event_publication_slots(),
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
    device: &DeviceExecution,
    queue_control: &QueueControlState,
    queue: QueueId,
) -> bool {
    device.queue(queue).is_ok_and(|context| {
        context.io.completion_blocked(
            queue_control,
            queue,
            device.execution.resources().completion_publication_slots(),
        )
    })
}
