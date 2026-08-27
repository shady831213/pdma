use std::sync::Arc;

use hardware_interface_runtime::{
    ObservationGate, RegisterObserver, SharedMemory, SharedRegisterObserver,
};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig, WholeDeviceReset};

use super::abi::bus::control::BusControlState;
use super::abi::mmio::RegisterBanks;
use super::execution::DeviceExecution;
use super::{AsyncStepStatus, PdmaMemoryMappings};

/// Sole device-owned state consumed by one asynchronous or synchronous facade.
///
/// A core cannot be installed in both facade kinds because `from_core` consumes it:
///
/// ```compile_fail
/// use std::sync::Arc;
/// use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory};
/// use pdma_model_contract::semantic::ResourceConfig;
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
    pub(super) backend: DeviceExecution,
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
    reset_runner_calls: usize,
    #[cfg(test)]
    pub(super) batch_fault: std::cell::Cell<Option<super::abi::bus::BatchPreparationFault>>,
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
            backend: DeviceExecution::with_resource_config(config),
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
            reset_runner_calls: 0,
            #[cfg(test)]
            batch_fault: std::cell::Cell::new(None),
        }
    }

    /// Atomically resets all resettable runner state through the sole reset seam.
    pub(crate) fn reset_runner(
        &mut self,
        request: WholeDeviceReset,
    ) -> Result<DeviceEpoch, super::abi::bus::PdmaControlError> {
        let backend_epoch = self.backend.epoch();
        let control_epoch = self.bus_control.epoch();
        if request.expected_epoch() != backend_epoch || control_epoch != backend_epoch {
            return Err(super::abi::bus::PdmaControlError::StaleEpoch {
                expected: backend_epoch,
                actual: request.expected_epoch(),
            });
        }
        let next_backend = backend_epoch.next()?;
        let next_control = control_epoch.next()?;
        if next_backend != next_control {
            return Err(super::abi::bus::PdmaControlError::InvalidTransaction);
        }
        self.backend.reset(next_backend);
        self.bus_control.reset(next_control);
        super::abi::mmio::reset::register_banks(self);
        self.reset_runner_calls = self.reset_runner_calls.saturating_add(1);
        Ok(next_backend)
    }

    /// Returns committed invocations of the sole reset runner.
    pub const fn reset_runner_calls(&self) -> usize {
        self.reset_runner_calls
    }
}
