use super::{PendingRetirement, RetirementPorts, RetirementStatus, RetirementUnit};
use crate::device::flow::SuccessfulExecution;
use crate::queue::io::PublicationOutcome;
use crate::status::AsyncStallReason;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::AbiCompletion;

impl SuccessfulExecution {
    pub(in crate::device) fn completion(self) -> ModelResult<AbiCompletion> {
        let queue =
            u64::try_from(self.route.queue.raw()).map_err(|_| ModelError::DmaSubmissionState)?;
        let command =
            u64::try_from(self.route.command.raw()).map_err(|_| ModelError::DmaSubmissionState)?;
        Ok(AbiCompletion::success(queue, command))
    }
}

impl RetirementUnit {
    pub(super) fn retire_success(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        success: SuccessfulExecution,
    ) -> ModelResult<RetirementStatus> {
        let route = success.route;
        let completion = success.completion()?;
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
        ports.retire_command(route.queue)?;
        Ok(RetirementStatus::Progressed)
    }

    pub(super) fn retry_completion(
        &mut self,
        ports: &mut RetirementPorts<'_>,
        pending: SuccessfulExecution,
    ) -> ModelResult<RetirementStatus> {
        let route = pending.route;
        let completion = pending.completion()?;
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
        self.complete();
        ports.requeue(route.queue)?;
        Ok(RetirementStatus::Progressed)
    }
}
