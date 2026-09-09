use crate::queue::QueueBlock;
use crate::queue::runnable::RunnableQueues;
use crate::trace::TraceSink;
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_contract::semantic::ResourceConfig;

pub(super) struct HardwareContext<'a> {
    pub(super) queues: &'a mut QueueBlock,
    pub(super) runnable: &'a mut RunnableQueues,
    pub(super) memory: &'a SharedMemoryAccess,
    pub(super) trace: &'a mut TraceSink,
    pub(super) fetches: &'a mut usize,
    pub(super) resources: ResourceConfig,
}
