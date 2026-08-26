use super::{DeviceExecution, PdmaIrqController};
use crate::runner::abi::bus::control::BusControlState;
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqVector};

impl DeviceExecution {
    pub(in crate::runner) fn reset_partition_counts(
        &self,
        control: &BusControlState,
    ) -> (usize, usize, usize, usize, usize) {
        let result_queues = self
            .queues
            .iter()
            .filter(|queue| {
                queue
                    .frontend
                    .records_pending(control, queue.entry.queue_id(), true)
                    .unwrap_or(false)
                    || queue
                        .frontend
                        .records_pending(control, queue.entry.queue_id(), false)
                        .unwrap_or(false)
            })
            .count();
        (
            self.queues.len(),
            result_queues,
            self.runnable.len(),
            usize::from(self.pending_retirement.is_some()),
            self.irq.source_count(),
        )
    }

    pub(in crate::runner) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.queues.clear();
        self.runnable.clear();
        self.pending_retirement = None;
        self.irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    }
}
