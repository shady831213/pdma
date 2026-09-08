use super::DeviceCore;
use crate::irq::PdmaIrqController;
use crate::queue::exact::PdmaControlError;
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqVector, WholeDeviceReset};

impl DeviceCore {
    pub(crate) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.command_pipeline.clear();
        self.queues.clear();
        self.runnable.reset();
        self.retirement.reset();
        self.irq = PdmaIrqController::new(epoch, IrqLine::new(IrqVector::new(0)));
    }

    pub(crate) fn reset_runner(
        &mut self,
        request: WholeDeviceReset,
    ) -> Result<DeviceEpoch, PdmaControlError> {
        let epoch = self.epoch();
        if request.expected_epoch() != epoch {
            return Err(PdmaControlError::StaleEpoch {
                expected: epoch,
                actual: request.expected_epoch(),
            });
        }
        let next = epoch.next()?;
        self.reset(next);
        self.global_registers = pdma_model_contract::abi::mmio::GlobalRegistersValues::default();
        self.reset_runner_calls = self.reset_runner_calls.saturating_add(1);
        Ok(next)
    }
}
