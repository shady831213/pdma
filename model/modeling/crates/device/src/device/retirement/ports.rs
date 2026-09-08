use crate::device::context::HardwareContext;
use crate::device::flow::ExecutionFault;
use crate::memory::PdmaMemoryMappings;
use crate::queue::QueueBlock;
use crate::queue::io::{FaultPublication, PublicationOutcome};
use crate::queue::runnable::RunnableQueues;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::AbiCompletion;
use pdma_model_contract::semantic::{QueueId, ResourceConfig};

pub(crate) struct RetirementPorts<'a> {
    pub(super) queues: &'a mut QueueBlock,
    pub(super) memory: &'a PdmaMemoryMappings,
    pub(super) runnable: &'a mut RunnableQueues,
    pub(super) trace: &'a mut TraceSink,
    pub(super) resources: ResourceConfig,
}

impl<'a> RetirementPorts<'a> {
    pub(crate) const fn new(context: &'a mut HardwareContext<'_>) -> Self {
        Self {
            queues: context.queues,
            memory: context.memory,
            runnable: context.runnable,
            trace: context.trace,
            resources: context.resources,
        }
    }

    pub(super) fn consume(&mut self, queue: QueueId) -> ModelResult<()> {
        self.queues.consume(queue)
    }

    pub(super) fn retire_command(&mut self, queue: QueueId) -> ModelResult<()> {
        self.consume(queue)?;
        self.requeue(queue)
    }

    pub(super) fn requeue(&mut self, queue: QueueId) -> ModelResult<()> {
        if self.queues.has_pending(queue)? {
            self.runnable.enqueue(queue);
        }
        Ok(())
    }

    pub(super) fn publish_completion(
        &mut self,
        queue: QueueId,
        record: AbiCompletion,
    ) -> ModelResult<PublicationOutcome> {
        self.queues
            .publish_completion(self.memory, queue, record, self.trace)
    }

    pub(super) fn publish_fault(&mut self, queue: QueueId) -> ModelResult<PublicationOutcome> {
        match self.queues.publish_fault(self.memory, queue, self.trace)? {
            FaultPublication::Retry => Ok(PublicationOutcome::Retry),
            FaultPublication::Exact(_) => Ok(PublicationOutcome::Published),
        }
    }

    pub(super) fn stage_fault(&mut self, fault: ExecutionFault) -> ModelResult<ModelError> {
        let queue = u64::try_from(fault.queue.raw()).map_err(|_| ModelError::DmaSubmissionState)?;
        let command =
            u64::try_from(fault.command.raw()).map_err(|_| ModelError::DmaSubmissionState)?;
        self.queues.stage_execution_fault(
            fault.queue,
            pdma_model_contract::abi::AbiEvent::new(queue, command, fault.exact),
        )?;
        Ok(fault
            .buffer
            .map_or(ModelError::DmaSubmissionState, |buffer| {
                ModelError::from_memory_fault(buffer, fault.error_kind)
            }))
    }
}
