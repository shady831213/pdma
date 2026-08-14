use super::core::DeviceStateSnapshot;
use super::{AsyncDeviceHandle, PdmaMemoryError};

impl AsyncDeviceHandle {
    /// Snapshots typed device state for integration-test support only.
    #[doc(hidden)]
    pub async fn state_snapshot_for_test(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
        self.shared.state.lock().await.state_snapshot_for_test()
    }
}
