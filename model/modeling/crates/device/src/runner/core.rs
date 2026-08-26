use std::sync::Arc;

use hardware_interface_runtime::{
    ObservationGate, RegisterObserver, SharedMemory, SharedRegisterObserver,
};
use pdma_model_contract::abi::mmio::PdmaMmioMap;
use pdma_model_contract::binding::PdmaMemoryRegion;
use pdma_model_contract::semantic::trace::TraceEvent;
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig, WholeDeviceReset};

use super::abi::bus::control::BusControlState;
use super::abi::mmio::RegisterBanks;
use super::execution::DeviceExecution;
use super::{AsyncRunSnapshot, AsyncStepStatus, PdmaMemoryError, PdmaMemoryMappings};

/// Direct, named reset partitions exposed solely to integration-test support.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetPartitionSnapshot {
    /// Number of configured queues in the canonical backend.
    pub queues: usize,
    /// Number of queues with a completion or event record pending.
    pub results: usize,
    /// Number of canonical runnable work entries.
    pub work: usize,
    /// Number of pending retirement entries.
    pub retirement: usize,
    /// Number of directly stored IRQ source states.
    pub irq_sources: usize,
    /// Direct reads of every canonical MMIO register in generated declaration order.
    pub registers: Vec<Vec<u8>>,
    /// Canonical backend epoch.
    pub backend_epoch: DeviceEpoch,
    /// Canonical typed-control epoch.
    pub control_epoch: DeviceEpoch,
}

/// Complete typed core state exposed solely to integration-test support.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceStateSnapshot {
    /// All logical DMA mappings in object order.
    pub mappings: Vec<PdmaMemoryRegion>,
    /// Exact bytes for every logical DMA mapping in object order.
    pub bytes: Vec<Vec<u8>>,
    /// Persistent DMA allocator generation.
    pub memory_generation: u64,
    /// Persistent resource configuration.
    pub resources: ResourceConfig,
    /// Persistent trace history.
    pub trace: Vec<TraceEvent>,
    /// Complete runner lifecycle state.
    pub runner: AsyncRunSnapshot,
    /// Monotonic publication generation held by the runner core.
    pub runner_generation: usize,
    /// Resettable backend/control/MMIO partitions.
    pub partitions: ResetPartitionSnapshot,
    /// Current PDMA-local IRQ state.
    pub irq: Option<pdma_model_contract::semantic::IrqSnapshot>,
    /// Committed calls to the sole reset runner.
    pub reset_runner_calls: usize,
}

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

    /// Constructs a core at an explicit epoch for integration-test support only.
    ///
    /// This remains a construction fixture: public reset operations still enter
    /// [`Self::reset_runner`] and therefore retain the canonical overflow path.
    #[doc(hidden)]
    pub fn new_at_epoch_for_test(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
        epoch: DeviceEpoch,
    ) -> Self {
        let mut core = Self::new(memory, config, observer);
        core.backend.reset(epoch);
        core.bus_control.reset(epoch);
        core
    }

    /// Constructs an epoch-seeded core with one typed writable MMIO commit for tests.
    #[doc(hidden)]
    pub fn new_at_epoch_with_mmio_write_for_test(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
        epoch: DeviceEpoch,
        offset: u32,
        request: &[u8],
    ) -> Result<Self, pdma_model_contract::abi::mmio::MmioAccessError> {
        let mut core = Self::new_at_epoch_for_test(memory, config, observer, epoch);
        let (register, value) = pdma_model_contract::abi::mmio::decode_write(offset, request)?;
        super::abi::register_endpoint::prepare_write(register, value, offset, request)
            .commit(&mut core);
        Ok(core)
    }

    /// Snapshots independent reset partitions without formatting or aggregation.
    #[doc(hidden)]
    pub fn reset_partition_snapshot_for_test(&self) -> ResetPartitionSnapshot {
        let (queues, results, work, retirement, irq_sources) =
            self.backend.reset_partition_counts(&self.bus_control);
        let registers = pdma_model_contract::abi::mmio::PdmaMmioMapRegisterId::ALL
            .into_iter()
            .filter_map(PdmaMmioMap::register)
            .map(|register| super::abi::mmio::dispatch::read(self, register.id()).encode())
            .collect();
        ResetPartitionSnapshot {
            queues,
            results,
            work,
            retirement,
            irq_sources,
            registers,
            backend_epoch: self.backend.epoch(),
            control_epoch: self.bus_control.epoch(),
        }
    }

    /// Snapshots every typed core partition without formatting or aggregation.
    #[doc(hidden)]
    pub fn state_snapshot_for_test(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
        let mappings = (0..self.dma_space.mapped_region_count())
            .map(|raw| {
                self.dma_space
                    .region_for_object(pdma_model_contract::semantic::BufferId::new(raw))
                    .ok_or(PdmaMemoryError::InvalidRegion)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bytes = mappings
            .iter()
            .map(|region| self.dma_space.read_memory(region.base(), region.length()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DeviceStateSnapshot {
            mappings,
            bytes,
            memory_generation: self.dma_space.generation(),
            resources: self.backend.resource_config(),
            trace: self.backend.trace_events(),
            runner: super::state::snapshot(self),
            runner_generation: self.generation,
            partitions: self.reset_partition_snapshot_for_test(),
            irq: self.backend.irq_snapshot(),
            reset_runner_calls: self.reset_runner_calls(),
        })
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
