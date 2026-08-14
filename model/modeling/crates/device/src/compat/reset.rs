use crate::PdmaResetError;
use crate::runner::{PdmaControlError, SyncDevice};
use pdma_model_contract::{DeviceEpoch, WholeDeviceReset};

impl SyncDevice {
    /// Atomically resets compatibility state through `DeviceCore::reset_runner`.
    pub fn reset(&mut self, request: WholeDeviceReset) -> Result<DeviceEpoch, PdmaResetError> {
        self.reset_core(request).map_err(PdmaResetError::from)
    }

    /// Transitional typed-control forwarding retained through Task 25.
    pub fn reset_control_compat(
        &mut self,
        request: WholeDeviceReset,
    ) -> Result<DeviceEpoch, PdmaControlError> {
        self.reset_core(request)
    }
}
