use super::AsyncDeviceHandle;
use super::backend::DeviceModel;
use super::native::probe::{RunStateProbe, probe_run_state};
use super::state::RunnerState;
use pdma_model_contract::{AbiDiscovery, DeviceRunState, DeviceStatus, DiscoveryRequest};

pub(in crate::runner) const fn discover(_request: DiscoveryRequest) -> AbiDiscovery {
    AbiDiscovery::v1()
}

pub(in crate::runner) fn status(state: &RunnerState) -> DeviceStatus {
    let run_state = match &state.backend {
        DeviceModel::QueuePort(device) => {
            probe_run_state(device, &state.dma_space, &state.bus_control)
        }
    };
    let run_state = match run_state {
        RunStateProbe::Idle => DeviceRunState::Idle,
        RunStateProbe::Running => DeviceRunState::Running,
        RunStateProbe::Blocked { .. } => DeviceRunState::Blocked,
    };
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
