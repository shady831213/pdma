use super::{PdmaIrqController, QueuePortDeviceModel};
use pdma_model_contract::{DeviceEpoch, IrqLine, IrqVector};

impl QueuePortDeviceModel {
    pub(in crate::runner) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.queues.clear();
        self.completions.clear();
        self.events.clear();
        self.runnable.clear();
        self.pending_retirement = None;
        self.irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    }
}
