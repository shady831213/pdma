use crate::PdmaResetError;
use crate::runtime::SyncDevice;
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};

impl SyncDevice {
    /// Atomically resets compatibility state through `DeviceCore::reset_runner`.
    pub fn reset(&mut self, request: WholeDeviceReset) -> Result<DeviceEpoch, PdmaResetError> {
        self.reset_core(request).map_err(PdmaResetError::from)
    }
}
