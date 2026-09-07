use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::abi::{AbiDiscovery, DiscoveryRequest};
use pdma_model_contract::semantic::DeviceStatus;

impl AsyncDeviceHandle {
    /// Discovers the ABI version and features accepted by this device.
    pub const fn discover(&self, _request: DiscoveryRequest) -> AbiDiscovery {
        AbiDiscovery::v1()
    }

    /// Returns the current typed device status snapshot.
    pub async fn current_status(&self) -> DeviceStatus {
        self.shared
            .backend
            .interact(|state| state.core.current_status())
            .await
    }
}
