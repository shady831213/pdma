use super::{DeviceExecution, PdmaIrqController};
use crate::abi::ring::control::QueueControlState;
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqVector};

impl DeviceExecution {
    pub(crate) fn reset_partition_counts(
        &self,
        control: &QueueControlState,
    ) -> (usize, usize, usize, usize, usize) {
        let result_queues = self
            .queues
            .iter()
            .filter(|queue| {
                queue
                    .io
                    .records_pending(control, queue.entry.queue_id(), true)
                    .unwrap_or(false)
                    || queue
                        .io
                        .records_pending(control, queue.entry.queue_id(), false)
                        .unwrap_or(false)
            })
            .count();
        (
            self.queues.len(),
            result_queues,
            self.scheduler.len(),
            usize::from(self.retirement.is_pending()),
            self.irq.source_count(),
        )
    }

    pub(crate) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.queues.clear();
        self.scheduler.reset();
        self.retirement.reset();
        self.irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    }
}
