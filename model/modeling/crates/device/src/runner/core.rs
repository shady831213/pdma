use std::sync::Arc;

use hardware_interface_runtime::{
    ObservationGate, RegisterObserver, SharedMemory, SharedRegisterObserver,
};
use pdma_model_contract::ResourceConfig;

use super::backend::DeviceModel;
use super::bus::control::BusControlState;
use super::mmio::RegisterBanks;
use super::{AsyncStepStatus, PdmaMemoryMappings};

/// Sole device-owned state consumed by one asynchronous or synchronous facade.
///
/// A core cannot be installed in both facade kinds because `from_core` consumes it:
///
/// ```compile_fail
/// use std::sync::Arc;
/// use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory};
/// use pdma_model_contract::ResourceConfig;
/// use pdma_model_device::{AsyncDevice, DeviceCore};
///
/// let core = DeviceCore::new(
///     Arc::new(SharedMemory::new()),
///     ResourceConfig::default(),
///     Arc::new(NoopRegisterObserver),
/// );
/// let _async_facade = AsyncDevice::from_core(core);
/// let _sync_facade = pdma_model_device::compat::SyncDevice::from_core(core);
/// ```
#[derive(Debug)]
pub struct DeviceCore {
    pub(super) backend: DeviceModel,
    pub(super) dma_space: PdmaMemoryMappings,
    pub(super) bus_control: BusControlState,
    pub(super) mmio: RegisterBanks,
    pub(super) steps: usize,
    pub(super) errors: usize,
    pub(super) stalls: usize,
    pub(super) last_status: AsyncStepStatus,
    pub(super) generation: usize,
    pub(super) wake_generation: usize,
    pub(super) shutdown_requested: bool,
    pub(super) register_gate: ObservationGate,
    #[cfg(test)]
    pub(super) batch_fault: std::cell::Cell<Option<super::bus::BatchPreparationFault>>,
}

impl DeviceCore {
    /// Creates one core over the system-owned memory and register observer capabilities.
    pub fn new(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
    ) -> Self {
        let observer = SharedRegisterObserver::new(observer);
        Self {
            backend: DeviceModel::queue_port_with_resource_config(config),
            dma_space: PdmaMemoryMappings::new(memory),
            bus_control: BusControlState::new(),
            mmio: RegisterBanks::new(),
            steps: 0,
            errors: 0,
            stalls: 0,
            last_status: AsyncStepStatus::Idle,
            generation: 0,
            wake_generation: 0,
            shutdown_requested: false,
            register_gate: ObservationGate::new(observer),
            #[cfg(test)]
            batch_fault: std::cell::Cell::new(None),
        }
    }
}
