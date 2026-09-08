use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::semantic::DeviceStatus;

impl AsyncDeviceHandle {
    /// Returns the current typed device status snapshot.
    pub async fn current_status(&self) -> DeviceStatus {
        self.shared
            .backend
            .interact(|state| state.core.current_status())
            .await
    }
}
