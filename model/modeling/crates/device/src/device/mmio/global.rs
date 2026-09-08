use super::DeviceCore;
use crate::queue::exact::PdmaControlError;
use pdma_model_contract::abi::registers::{GlobalRegisters, GlobalRegistersValues};
use pdma_model_contract::semantic::DeviceRunState;

impl DeviceCore {
    pub(super) fn global_values(&self) -> GlobalRegistersValues {
        GlobalRegistersValues {
            epoch: self.epoch().raw().into(),
            queue_count: self.queues.queue_count().into(),
            run_state: match self.run_state() {
                DeviceRunState::Idle => 0,
                DeviceRunState::Running => 1,
                DeviceRunState::Blocked => 2,
            }
            .into(),
            ..GlobalRegistersValues::default()
        }
    }

    pub(super) fn write_global(
        &mut self,
        offset: usize,
        value: u64,
    ) -> Result<bool, PdmaControlError> {
        if offset != GlobalRegisters::RESET.offset() || value != 1 {
            return Err(PdmaControlError::InvalidTransaction);
        }
        self.reset_runner(self.epoch())?;
        Ok(true)
    }
}
