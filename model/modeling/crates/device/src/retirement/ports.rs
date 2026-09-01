use crate::dma::PdmaMemoryMappings;
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::queue::io::{FaultPublication, PublicationOutcome};
use crate::scheduler::Scheduler;
use crate::trace::TraceSink;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    CompletionRecord, EventRecord, IrqSource, MemoryFault, QueueId, ResourceConfig,
};

pub(crate) struct RetirementPorts<'a> {
    pub(super) queues: &'a mut QueueBlock,
    pub(super) dma: &'a PdmaMemoryMappings,
    pub(super) scheduler: &'a mut Scheduler,
    pub(super) irq: &'a mut PdmaIrqController,
    pub(super) trace: &'a mut TraceSink,
    pub(super) resources: ResourceConfig,
}

impl<'a> RetirementPorts<'a> {
    pub(crate) const fn new(
        queues: &'a mut QueueBlock,
        dma: &'a PdmaMemoryMappings,
        scheduler: &'a mut Scheduler,
        irq: &'a mut PdmaIrqController,
        trace: &'a mut TraceSink,
        resources: ResourceConfig,
    ) -> Self {
        Self {
            queues,
            dma,
            scheduler,
            irq,
            trace,
            resources,
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
            self.scheduler.enqueue(queue);
        }
        Ok(())
    }

    pub(super) fn publish_completion(
        &mut self,
        queue: QueueId,
        record: CompletionRecord,
    ) -> ModelResult<PublicationOutcome> {
        self.queues.publish_completion(self.dma, queue, record)
    }

    pub(super) fn publish_fault(&mut self, queue: QueueId) -> ModelResult<PublicationOutcome> {
        match self.queues.publish_fault(self.dma, queue)? {
            FaultPublication::Retry => Ok(PublicationOutcome::Retry),
            FaultPublication::Semantic(event) => {
                self.trace.event_published(event);
                Ok(PublicationOutcome::Published)
            }
            FaultPublication::Exact(trace) => {
                if let Some((queue, command)) = trace {
                    self.trace.dma_event_published(queue, command);
                }
                Ok(PublicationOutcome::Published)
            }
        }
    }

    pub(super) fn stage_fault(
        &mut self,
        queue: QueueId,
        event: EventRecord,
    ) -> ModelResult<ModelError> {
        self.queues.stage_execution_fault(queue, event)
    }

    pub(super) fn latch(&mut self, source: IrqSource) {
        let was_asserted = self.irq.line_asserted();
        if self.irq.latch(source) {
            self.trace.irq_source_latched(source);
        }
        match (was_asserted, self.irq.line_asserted()) {
            (false, true) => self.trace.irq_line_asserted(),
            (true, false) => self.trace.irq_line_deasserted(),
            (false, false) | (true, true) => {}
        }
    }

    pub(super) const fn model_error(fault: MemoryFault) -> ModelError {
        ModelError::from_memory_fault(fault.buffer(), fault.error_kind())
    }
}
