use super::{PendingRetirement, RetirementPorts, RetirementStatus, RetirementUnit};
use crate::device::flow::ExecutionFault;
use crate::queue::io::PublicationOutcome;
use crate::status::AsyncStallReason;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::QueueId;

impl RetirementUnit {
    pub(super) fn retire_fault(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        fault: ExecutionFault,
    ) -> ModelResult<RetirementStatus> {
        ports
            .trace
            .execution_faulted(fault.queue, fault.command, fault.error_kind);
        let source = ports.stage_fault(fault)?;
        if ports
            .queues
            .fault_blocked(fault.queue, ports.resources.event_publication_slots())?
        {
            ports.consume(fault.queue)?;
            self.hold(PendingRetirement::Fault {
                queue: fault.queue,
                source: Some(source),
            });
            ports
                .trace
                .stall(Some(fault.queue), "EventPublicationPressure");
            return Ok(RetirementStatus::Stalled(
                AsyncStallReason::EventPublicationPressure,
            ));
        }
        if ports.publish_fault(fault.queue)? == PublicationOutcome::Retry {
            ports.consume(fault.queue)?;
            self.hold(PendingRetirement::Fault {
                queue: fault.queue,
                source: Some(source),
            });
            return Ok(RetirementStatus::Stalled(
                AsyncStallReason::EventPublicationPressure,
            ));
        }
        ports.retire_command(fault.queue)?;
        Err(source)
    }

    pub(super) fn retry_fault(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        queue: QueueId,
        source: Option<ModelError>,
    ) -> ModelResult<RetirementStatus> {
        let (reason, detail) = if std::mem::take(&mut self.fault_unattempted) {
            (
                AsyncStallReason::EventPublicationPressure,
                "EventPublicationPressure",
            )
        } else {
            (
                AsyncStallReason::FaultRetirementPendingRetry,
                "FaultRetirementPendingRetry",
            )
        };
        if ports
            .queues
            .fault_blocked(queue, ports.resources.event_publication_slots())?
        {
            ports.trace.stall(Some(queue), detail);
            return Ok(RetirementStatus::Stalled(reason));
        }
        if ports.publish_fault(queue)? == PublicationOutcome::Retry {
            return Ok(RetirementStatus::Stalled(reason));
        }
        self.complete();
        ports.requeue(queue)?;
        source.map_or(Ok(RetirementStatus::Progressed), Err)
    }
}
