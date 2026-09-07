use super::{PendingRetirement, RetirementPorts, RetirementStatus, RetirementUnit};
use crate::device::execution::ExecutionFault;
use crate::queue::io::PublicationOutcome;
use crate::status::AsyncStallReason;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{EventRecord, IrqSource, QueueId};

impl RetirementUnit {
    pub(super) fn retire_fault(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        fault: ExecutionFault,
    ) -> ModelResult<RetirementStatus> {
        let event = EventRecord::from_memory_fault(fault.queue, fault.command, fault.fault);
        ports.trace.execution_faulted(event);
        let source = ports.stage_fault(fault.queue, event)?;
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
        ports.latch(IrqSource::Event(fault.queue));
        ports.retire_command(fault.queue)?;
        Err(RetirementPorts::model_error(fault.fault))
    }

    pub(super) fn retry_fault(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        queue: QueueId,
        source: Option<ModelError>,
    ) -> ModelResult<RetirementStatus> {
        if ports
            .queues
            .fault_blocked(queue, ports.resources.event_publication_slots())?
        {
            ports
                .trace
                .stall(Some(queue), "FaultRetirementPendingRetry");
            return Ok(RetirementStatus::Stalled(
                AsyncStallReason::FaultRetirementPendingRetry,
            ));
        }
        if ports.publish_fault(queue)? == PublicationOutcome::Retry {
            return Ok(RetirementStatus::Stalled(
                AsyncStallReason::FaultRetirementPendingRetry,
            ));
        }
        ports.latch(IrqSource::Event(queue));
        self.complete();
        ports.requeue(queue)?;
        source.map_or(Ok(RetirementStatus::Progressed), Err)
    }
}
