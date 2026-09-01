use super::DeviceCore;
use super::probe::{RunStateProbe, probe_run_state};
use crate::execution::CommandRoute;
use crate::queue::io::NormalizedFetch;
use crate::retirement::{RetirementPorts, RetirementStatus};
use crate::status::AsyncStallReason;
use crate::{ModelError, ModelResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelStepStatus {
    Progressed,
    FaultCached,
    Idle,
    Stalled(AsyncStallReason),
}

impl DeviceCore {
    pub(crate) const fn resource_config(&self) -> pdma_model_contract::semantic::ResourceConfig {
        self.execution.resources()
    }

    pub(crate) fn step(&mut self) -> ModelResult<ModelStepStatus> {
        match probe_run_state(
            &self.queues,
            &self.dma,
            &self.scheduler,
            &self.execution,
            &self.retirement,
        ) {
            RunStateProbe::Idle => {
                self.trace.idle();
                return Ok(ModelStepStatus::Idle);
            }
            RunStateProbe::Running
            | RunStateProbe::Blocked {
                reason: AsyncStallReason::ExecutionResourcePressure,
                ..
            } => {}
            RunStateProbe::Blocked { queue, reason } => {
                self.trace.stall(Some(queue), stall_name(reason));
                return Ok(ModelStepStatus::Stalled(reason));
            }
        }
        if self.retirement.is_pending() {
            let status = self.retry_retirement()?;
            return Ok(map_retirement_status(status));
        }
        let Some(queue) = self.scheduler.next() else {
            return Ok(ModelStepStatus::Idle);
        };
        let Some(fetched) = self.queues.fetch(&self.dma, queue)? else {
            return Ok(ModelStepStatus::Progressed);
        };
        self.execution.record_fetch();
        let (command, completion_ring) = match fetched {
            NormalizedFetch::Command {
                command,
                completion_ring,
            } => (command, completion_ring),
            NormalizedFetch::Fault(fault) => {
                let status = self.cache_frontend_fault(fault)?;
                return Ok(map_retirement_status(status));
            }
        };
        let operation = command.operation().trace_operation();
        let command_id = command.command_id();
        self.trace.scheduled(queue, operation, command_id);
        let route = CommandRoute::new(
            queue,
            command_id,
            completion_ring,
            self.queues.contains_completion(completion_ring),
        );
        let outcome = match self.execution.execute(&self.dma, route, command) {
            Ok(outcome) => outcome,
            Err(reason) => {
                self.requeue(queue)?;
                self.trace.stall(Some(queue), stall_name(reason));
                return Ok(ModelStepStatus::Stalled(reason));
            }
        };
        let retirement = match outcome.into_retirement() {
            Ok(retirement) => retirement,
            Err(route) => {
                self.queues.consume(route.queue)?;
                self.requeue(route.queue)?;
                return Err(ModelError::UnknownCompletionRing(route.completion_ring));
            }
        };
        let status = self.retire(retirement)?;
        Ok(map_retirement_status(status))
    }

    pub(crate) fn has_work(&self) -> bool {
        self.retirement.is_pending() || !self.scheduler.is_idle() || self.queues.has_any_pending()
    }

    fn retire(
        &mut self,
        outcome: crate::execution::RetirableExecution,
    ) -> ModelResult<RetirementStatus> {
        let resources = self.execution.resources();
        let mut ports = RetirementPorts::new(
            &mut self.queues,
            &self.dma,
            &mut self.scheduler,
            &mut self.irq,
            &mut self.trace,
            resources,
        );
        self.retirement.retire(&mut ports, outcome)
    }

    fn retry_retirement(&mut self) -> ModelResult<RetirementStatus> {
        let resources = self.execution.resources();
        let mut ports = RetirementPorts::new(
            &mut self.queues,
            &self.dma,
            &mut self.scheduler,
            &mut self.irq,
            &mut self.trace,
            resources,
        );
        self.retirement.retry(&mut ports)
    }

    fn cache_frontend_fault(
        &mut self,
        fault: crate::queue::io::NormalizedFault,
    ) -> ModelResult<RetirementStatus> {
        let resources = self.execution.resources();
        let mut ports = RetirementPorts::new(
            &mut self.queues,
            &self.dma,
            &mut self.scheduler,
            &mut self.irq,
            &mut self.trace,
            resources,
        );
        self.retirement.cache_frontend_fault(&mut ports, fault)
    }

    fn requeue(&mut self, queue: pdma_model_contract::semantic::QueueId) -> ModelResult<()> {
        if self.queues.has_pending(queue)? {
            self.scheduler.enqueue(queue);
        }
        Ok(())
    }
}

const fn map_retirement_status(status: RetirementStatus) -> ModelStepStatus {
    match status {
        RetirementStatus::Progressed => ModelStepStatus::Progressed,
        RetirementStatus::FaultCached => ModelStepStatus::FaultCached,
        RetirementStatus::Stalled(reason) => ModelStepStatus::Stalled(reason),
    }
}

const fn stall_name(reason: AsyncStallReason) -> &'static str {
    match reason {
        AsyncStallReason::ExecutionResourcePressure => "ExecutionResourcePressure",
        AsyncStallReason::CompletionPublicationPressure => "CompletionPublicationPressure",
        AsyncStallReason::EventPublicationPressure => "EventPublicationPressure",
        AsyncStallReason::FaultRetirementPendingRetry => "FaultRetirementPendingRetry",
    }
}
