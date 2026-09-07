use crate::queue::exact::PdmaControlError;
use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};

impl AsyncDeviceHandle {
    /// Atomically resets device-owned state and advances recovery identity.
    pub async fn reset(&self, request: WholeDeviceReset) -> Result<DeviceEpoch, PdmaControlError> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.reset_runner(request);
                let changed = result.is_ok();
                (result, changed)
            })
            .await
    }
}
