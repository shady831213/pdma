use crate::dma::PdmaMemoryMappings;
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::scheduler::RunnableQueues;
use crate::trace::TraceSink;
use pdma_model_contract::semantic::ResourceConfig;

pub(crate) struct HardwareContext<'a> {
    pub(crate) queues: &'a mut QueueBlock,
    pub(crate) scheduler: &'a mut RunnableQueues,
    pub(crate) dma: &'a PdmaMemoryMappings,
    pub(crate) irq: &'a mut PdmaIrqController,
    pub(crate) trace: &'a mut TraceSink,
    pub(crate) fetches: &'a mut usize,
    pub(crate) resources: ResourceConfig,
}
