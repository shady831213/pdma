use std::sync::Arc;

use crate::dma::MappedDmaRegion;
use hardware_interface_runtime::{RegisterObserver, SharedMemory};
use pdma_model_contract::abi::mmio::MmioAccessError;
use pdma_model_contract::semantic::trace::TraceEvent;
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};

use crate::{AsyncDeviceHandle, AsyncRunSnapshot, DeviceCore, PdmaMemoryError, SyncDevice};

/// Direct reset partitions captured only by the explicit test-support API.
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

/// Complete typed core state captured only by the explicit test-support API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceStateSnapshot {
    /// All logical DMA mappings in object order.
    pub mappings: Vec<MappedDmaRegion>,
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

pub(crate) trait CoreTestSupport {
    fn at_epoch(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
        epoch: DeviceEpoch,
    ) -> DeviceCore;
    fn at_epoch_with_mmio_write(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
        epoch: DeviceEpoch,
        offset: u32,
        request: &[u8],
    ) -> Result<DeviceCore, MmioAccessError>;
    fn reset_partition_snapshot(&self) -> ResetPartitionSnapshot;
    fn state_snapshot(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError>;
}

pub(crate) trait AsyncTestSupport {
    async fn state_snapshot(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError>;
}

pub(crate) trait SyncTestSupport {
    fn state_snapshot(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError>;
}

/// Builds an epoch-seeded core for integration fixtures.
pub fn device_core_at_epoch(
    memory: Arc<SharedMemory>,
    config: ResourceConfig,
    observer: Arc<dyn RegisterObserver>,
    epoch: DeviceEpoch,
) -> DeviceCore {
    <DeviceCore as CoreTestSupport>::at_epoch(memory, config, observer, epoch)
}

/// Builds an epoch-seeded core and commits one writable MMIO register for a fixture.
pub fn device_core_at_epoch_with_mmio_write(
    memory: Arc<SharedMemory>,
    config: ResourceConfig,
    observer: Arc<dyn RegisterObserver>,
    epoch: DeviceEpoch,
    offset: u32,
    request: &[u8],
) -> Result<DeviceCore, MmioAccessError> {
    <DeviceCore as CoreTestSupport>::at_epoch_with_mmio_write(
        memory, config, observer, epoch, offset, request,
    )
}

/// Captures independent reset partitions without formatting or aggregation.
pub fn reset_partition_snapshot(core: &DeviceCore) -> ResetPartitionSnapshot {
    CoreTestSupport::reset_partition_snapshot(core)
}

/// Captures every typed core partition without formatting or aggregation.
pub fn state_snapshot(core: &DeviceCore) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
    CoreTestSupport::state_snapshot(core)
}

/// Captures an async runner state through the explicit test-support owner.
pub async fn async_state_snapshot(
    handle: &AsyncDeviceHandle,
) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
    AsyncTestSupport::state_snapshot(handle).await
}

/// Captures a synchronous facade state through the explicit test-support owner.
pub fn sync_state_snapshot(device: &SyncDevice) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
    SyncTestSupport::state_snapshot(device)
}
