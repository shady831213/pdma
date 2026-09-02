mod completion;
mod fault;
#[doc(hidden)]
pub(crate) mod ports;

pub(crate) use ports::RetirementPorts;

use crate::ModelResult;
use crate::execution::{RetirableExecution, SuccessfulExecution};
use crate::queue::io::NormalizedFault;
use crate::status::AsyncStallReason;
use pdma_model_contract::semantic::QueueId;

#[derive(Debug, Clone, Copy)]
pub(crate) enum PendingRetirement {
    Completion(SuccessfulExecution),
    Fault {
        queue: QueueId,
        source: Option<crate::ModelError>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetirementStatus {
    Progressed,
    FaultCached,
    Stalled(AsyncStallReason),
}

#[derive(Debug, Default)]
pub(crate) struct RetirementUnit {
    pending: Option<PendingRetirement>,
}

impl RetirementUnit {
    pub(crate) const fn pending(&self) -> Option<PendingRetirement> {
        self.pending
    }

    pub(crate) const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn retire(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        outcome: RetirableExecution,
    ) -> ModelResult<RetirementStatus> {
        match outcome {
            RetirableExecution::Success(success) => self.retire_success(ports, success),
            RetirableExecution::Fault(fault) => self.retire_fault(ports, fault),
        }
    }

    pub(crate) fn retry(
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

    pub(crate) fn cache_frontend_fault(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        fault: NormalizedFault,
    ) -> ModelResult<RetirementStatus> {
        ports.consume(fault.queue)?;
        self.hold(PendingRetirement::Fault {
            queue: fault.queue,
            source: Some(fault.source),
        });
        Ok(RetirementStatus::FaultCached)
    }

    pub(crate) const fn reset(&mut self) {
        self.pending = None;
    }

    const fn hold(&mut self, pending: PendingRetirement) {
        self.pending = Some(pending);
    }

    const fn complete(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
mod tests;
