use super::AsyncDeviceHandle;
use super::state::DeviceCore;
use pdma_model_contract::abi::{AbiDiscovery, DiscoveryRequest};
use pdma_model_contract::semantic::DeviceStatus;

pub(super) const fn discover(_request: DiscoveryRequest) -> AbiDiscovery {
    AbiDiscovery::v1()
}

pub(super) fn status(state: &DeviceCore) -> DeviceStatus {
    let run_state = state
        .backend
        .run_state(&state.dma_space, &state.queue_control);
    DeviceStatus::new(state.backend.epoch(), run_state)
}

impl AsyncDeviceHandle {
    /// Discovers the ABI version and features accepted by this device.
    pub const fn discover(&self, request: DiscoveryRequest) -> AbiDiscovery {
        discover(request)
    }

    /// Returns the current typed device status snapshot.
    pub async fn current_status(&self) -> DeviceStatus {
        let state = self.shared.state.lock().await;
        status(&state)
    }
}
