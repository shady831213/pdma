use super::DeviceCore;
use crate::irq::PdmaIrqController;
use crate::queue::exact::PdmaControlError;
use pdma_model_contract::semantic::DeviceEpoch;

impl DeviceCore {
    pub(crate) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.command_pipeline.clear();
        self.queues.clear();
        self.runnable.reset();
        self.command_pipeline.retirement.reset();
        self.irq = PdmaIrqController::new(epoch);
    }

    pub(crate) fn reset_runner(
        &mut self,
        expected_epoch: DeviceEpoch,
    ) -> Result<DeviceEpoch, PdmaControlError> {
        let epoch = self.epoch();
        if expected_epoch != epoch {
            return Err(PdmaControlError::StaleEpoch {
                expected: epoch,
                actual: expected_epoch,
            });
        }
        let next = epoch.next()?;
        self.reset(next);
        self.reset_runner_calls = self.reset_runner_calls.saturating_add(1);
        Ok(next)
    }
}
