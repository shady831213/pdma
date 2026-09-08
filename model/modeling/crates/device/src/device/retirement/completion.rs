use super::{PendingRetirement, RetirementPorts, RetirementStatus, RetirementUnit};
use crate::ModelResult;
use crate::device::flow::SuccessfulExecution;
use crate::queue::io::PublicationOutcome;
use crate::status::AsyncStallReason;
use pdma_model_contract::semantic::{CompletionRecord, CompletionStatus, IrqSource};

impl RetirementUnit {
    pub(super) fn retire_success(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        success: SuccessfulExecution,
    ) -> ModelResult<RetirementStatus> {
        let route = success.route;
        let completion =
            CompletionRecord::new(route.queue, route.command, CompletionStatus::Success);
        ports
            .trace
            .execution_succeeded(route.queue, success.operation, route.command);
        if ports
            .queues
            .completion_blocked(route.queue, ports.resources.completion_publication_slots())?
        {
            ports.consume(route.queue)?;
            self.hold(PendingRetirement::Completion(success));
            ports
                .trace
                .stall(Some(route.queue), "CompletionPublicationPressure");
            return Ok(RetirementStatus::Stalled(
                AsyncStallReason::CompletionPublicationPressure,
            ));
        }
        if ports.publish_completion(route.queue, completion)? == PublicationOutcome::Retry {
            ports.consume(route.queue)?;
            self.hold(PendingRetirement::Completion(success));
            ports
                .trace
                .stall(Some(route.queue), "CompletionPublicationPressure");
            return Ok(RetirementStatus::Stalled(
                AsyncStallReason::CompletionPublicationPressure,
            ));
        }
        ports.trace.completion_published(completion);
        ports.latch(IrqSource::Completion(route.completion_ring));
        ports.retire_command(route.queue)?;
        Ok(RetirementStatus::Progressed)
    }

    pub(super) fn retry_completion(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        pending: SuccessfulExecution,
    ) -> ModelResult<RetirementStatus> {
        let route = pending.route;
        let completion =
            CompletionRecord::new(route.queue, route.command, CompletionStatus::Success);
        if ports
            .queues
            .completion_blocked(route.queue, ports.resources.completion_publication_slots())?
            || ports.publish_completion(route.queue, completion)? == PublicationOutcome::Retry
        {
            ports
                .trace
                .stall(Some(route.queue), "CompletionPublicationPressure");
            return Ok(RetirementStatus::Stalled(
                AsyncStallReason::CompletionPublicationPressure,
            ));
        }
        ports.trace.completion_published(completion);
        ports.latch(IrqSource::Completion(route.completion_ring));
        self.complete();
        ports.requeue(route.queue)?;
        Ok(RetirementStatus::Progressed)
    }
}
