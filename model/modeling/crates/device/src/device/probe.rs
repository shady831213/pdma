use super::DeviceCore;
use crate::dma::PdmaMemoryMappings;
use crate::execution::ExecutionUnit;
use crate::queue::QueueBlock;
use crate::queue::io::NormalizedFetch;
use crate::retirement::{PendingRetirement, RetirementUnit};
use crate::scheduler::Scheduler;
use crate::status::AsyncStallReason;
use pdma_model_contract::semantic::{DeviceRunState, DeviceStatus, QueueId};

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
    queues: &QueueBlock,
    dma: &PdmaMemoryMappings,
    scheduler: &Scheduler,
    execution: &ExecutionUnit,
    retirement: &RetirementUnit,
) -> RunStateProbe {
    if let Some(pending) = retirement.pending() {
        return pending_probe(queues, execution, pending);
    }
    let Some(queue) = scheduler.current() else {
        return if queues.has_any_pending() {
            RunStateProbe::Running
        } else {
            RunStateProbe::Idle
        };
    };
    match queues.probe(dma, queue) {
        Ok(Some(NormalizedFetch::Command { command, .. })) if !execution.can_execute(&command) => {
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

impl DeviceCore {
    pub(crate) fn current_status(&self) -> DeviceStatus {
        DeviceStatus::new(self.epoch(), self.run_state())
    }

    pub(crate) fn run_state(&self) -> DeviceRunState {
        match probe_run_state(
            &self.queues,
            &self.dma,
            &self.scheduler,
            &self.execution,
            &self.retirement,
        ) {
            RunStateProbe::Idle => DeviceRunState::Idle,
            RunStateProbe::Running => DeviceRunState::Running,
            RunStateProbe::Blocked { .. } => DeviceRunState::Blocked,
        }
    }
}

fn pending_probe(
    queues: &QueueBlock,
    execution: &ExecutionUnit,
    pending: PendingRetirement,
) -> RunStateProbe {
    let (queue, reason, blocked) = match pending {
        PendingRetirement::Completion(success) => (
            success.route.queue,
            AsyncStallReason::CompletionPublicationPressure,
            queues
                .completion_blocked(
                    success.route.queue,
                    execution.resources().completion_publication_slots(),
                )
                .map_or(true, core::convert::identity),
        ),
        PendingRetirement::Fault { queue, .. } => (
            queue,
            AsyncStallReason::FaultRetirementPendingRetry,
            queues
                .fault_blocked(queue, execution.resources().event_publication_slots())
                .map_or(true, core::convert::identity),
        ),
    };
    if blocked {
        RunStateProbe::Blocked { queue, reason }
    } else {
        RunStateProbe::Running
    }
}
