mod activation;
use super::DeviceCore;
use crate::queue::exact::PendingDmaResult;
impl DeviceCore {
    pub(crate) fn pending_dma_result(&self) -> Option<PendingDmaResult> {
        match self.command_pipeline.retirement.pending() {
            Some(crate::device::retirement::PendingRetirement::Completion(success)) => {
                success.completion().ok().map(PendingDmaResult::Completion)
            }
            Some(crate::device::retirement::PendingRetirement::Fault { queue, .. }) => {
                self.queues.pending_exact_fault(queue)
            }
            None => None,
        }
    }
}
