mod completion;
mod fault;
mod ports;
pub(super) mod stage;

pub(super) use ports::RetirementPorts;

use crate::ModelResult;
use crate::device::flow::{RetirableExecution, SuccessfulExecution};
use crate::queue::io::NormalizedFault;
use crate::status::AsyncStallReason;
use pdma_model_contract::semantic::QueueId;

#[derive(Debug, Clone, Copy)]
pub(super) enum PendingRetirement {
    Completion(SuccessfulExecution),
    Fault {
        queue: QueueId,
        source: Option<crate::ModelError>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RetirementStatus {
    Progressed,
    FaultCached,
    Stalled(AsyncStallReason),
}

#[derive(Debug, Default)]
pub(super) struct RetirementUnit {
    pending: Option<PendingRetirement>,
    fault_unattempted: bool,
}

impl RetirementUnit {
    pub(super) const fn pending(&self) -> Option<PendingRetirement> {
        self.pending
    }

    #[cfg(test)]
    pub(super) const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn retire(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        outcome: RetirableExecution,
    ) -> ModelResult<RetirementStatus> {
        match outcome {
            RetirableExecution::Success(success) => self.retire_success(ports, success),
            RetirableExecution::Fault(fault) => self.retire_fault(ports, fault),
        }
    }

    pub(super) fn retry(
        &mut self,
        ports: &mut RetirementPorts<'_>,
    ) -> ModelResult<RetirementStatus> {
        match self.pending {
            Some(PendingRetirement::Completion(success)) => self.retry_completion(ports, success),
            Some(PendingRetirement::Fault { queue, source }) => {
                self.retry_fault(ports, queue, source)
            }
            None => Ok(RetirementStatus::Progressed),
        }
    }

    pub(super) fn cache_frontend_fault(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        fault: NormalizedFault,
    ) -> ModelResult<RetirementStatus> {
        ports.consume(fault.queue)?;
        self.hold(PendingRetirement::Fault {
            queue: fault.queue,
            source: Some(fault.source),
        });
        self.fault_unattempted = true;
        Ok(RetirementStatus::FaultCached)
    }

    pub(super) const fn reset(&mut self) {
        self.pending = None;
        self.fault_unattempted = false;
    }

    const fn hold(&mut self, pending: PendingRetirement) {
        self.pending = Some(pending);
    }

    const fn complete(&mut self) {
        self.pending = None;
        self.fault_unattempted = false;
    }
}

#[cfg(test)]
mod tests;
