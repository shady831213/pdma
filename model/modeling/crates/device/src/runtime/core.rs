use std::sync::Arc;

use hardware_interface_runtime::{
    ObservationGate, RegisterObserver, SharedMemory, SharedRegisterObserver,
};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig, WholeDeviceReset};

use super::{AsyncStepStatus, PdmaMemoryMappings};
use crate::abi::mmio::RegisterBanks;
use crate::abi::ring::control::QueueControlState;
use crate::engine::DeviceExecution;

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
    pub(crate) backend: DeviceExecution,
    pub(crate) dma_space: PdmaMemoryMappings,
    pub(crate) queue_control: QueueControlState,
    pub(crate) mmio: RegisterBanks,
    pub(crate) steps: usize,
    pub(crate) errors: usize,
    pub(crate) stalls: usize,
    pub(crate) last_status: AsyncStepStatus,
    pub(crate) generation: usize,
    pub(crate) wake_generation: usize,
    pub(crate) shutdown_requested: bool,
    pub(crate) register_gate: ObservationGate,
    reset_runner_calls: usize,
    #[cfg(test)]
    pub(crate) batch_fault: std::cell::Cell<Option<crate::abi::ring::BatchPreparationFault>>,
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
            queue_control: QueueControlState::new(),
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
    ) -> Result<DeviceEpoch, crate::abi::ring::PdmaControlError> {
        let backend_epoch = self.backend.epoch();
        let control_epoch = self.queue_control.epoch();
        if request.expected_epoch() != backend_epoch || control_epoch != backend_epoch {
            return Err(crate::abi::ring::PdmaControlError::StaleEpoch {
                expected: backend_epoch,
                actual: request.expected_epoch(),
            });
        }
        let next_backend = backend_epoch.next()?;
        let next_control = control_epoch.next()?;
        if next_backend != next_control {
            return Err(crate::abi::ring::PdmaControlError::InvalidTransaction);
        }
        self.backend.reset(next_backend);
        self.queue_control.reset(next_control);
        crate::abi::mmio::reset::register_banks(self);
        self.reset_runner_calls = self.reset_runner_calls.saturating_add(1);
        Ok(next_backend)
    }

    /// Returns committed invocations of the sole reset runner.
    pub const fn reset_runner_calls(&self) -> usize {
        self.reset_runner_calls
    }
}
