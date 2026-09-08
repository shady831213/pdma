use crate::irq::PdmaIrqController;
use crate::memory::PdmaMemoryMappings;
use crate::queue::QueueBlock;
use crate::queue::runnable::RunnableQueues;
use crate::trace::TraceSink;
use pdma_model_contract::semantic::ResourceConfig;

pub(super) struct HardwareContext<'a> {
    pub(super) queues: &'a mut QueueBlock,
    pub(super) runnable: &'a mut RunnableQueues,
    pub(super) dma: &'a PdmaMemoryMappings,
    pub(super) irq: &'a mut PdmaIrqController,
    pub(super) trace: &'a mut TraceSink,
    pub(super) fetches: &'a mut usize,
    pub(super) resources: ResourceConfig,
}
