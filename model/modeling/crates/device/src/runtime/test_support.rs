use std::sync::Arc;

use hardware_interface_runtime::{RegisterObserver, SharedMemory};
use pdma_model_contract::abi::mmio::{MmioAccessError, PdmaMmioMap, PdmaMmioMapRegisterId};
use pdma_model_contract::semantic::{BufferId, DeviceEpoch, ResourceConfig};

use crate::test_support::{
    AsyncTestSupport, CoreTestSupport, DeviceStateSnapshot, ResetPartitionSnapshot, SyncTestSupport,
};

use super::{AsyncDeviceHandle, DeviceCore, PdmaMemoryError, SyncDevice};

impl CoreTestSupport for DeviceCore {
    fn at_epoch(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
        epoch: DeviceEpoch,
    ) -> Self {
        let mut core = Self::new(memory, config, observer);
        core.device.execution.reset(epoch);
        core.device.queue_control.reset(epoch);
        core
    }

    fn at_epoch_with_mmio_write(
        memory: Arc<SharedMemory>,
        config: ResourceConfig,
        observer: Arc<dyn RegisterObserver>,
        epoch: DeviceEpoch,
        offset: u32,
        request: &[u8],
    ) -> Result<Self, MmioAccessError> {
        let mut core = Self::at_epoch(memory, config, observer, epoch);
        let (register, value) = pdma_model_contract::abi::mmio::decode_write(offset, request)?;
        crate::abi::endpoint::prepare_write(register, value, offset, request).commit(&mut core);
        Ok(core)
    }

    fn reset_partition_snapshot(&self) -> ResetPartitionSnapshot {
        let (queues, results, work, retirement, irq_sources) = self
            .device
            .execution
            .reset_partition_counts(&self.device.queue_control);
        let registers = PdmaMmioMapRegisterId::ALL
            .into_iter()
            .filter_map(PdmaMmioMap::register)
            .map(|register| crate::abi::mmio::dispatch::read(self, register.id()).encode())
            .collect();
        ResetPartitionSnapshot {
            queues,
            results,
            work,
            retirement,
            irq_sources,
            registers,
            backend_epoch: self.device.execution.epoch(),
            control_epoch: self.device.queue_control.epoch(),
        }
    }

    fn state_snapshot(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
        let mappings = (0..self.device.dma.mapped_region_count())
            .map(|raw| {
                self.device
                    .dma
                    .region_for_object(BufferId::new(raw))
                    .ok_or(PdmaMemoryError::InvalidRegion)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bytes = mappings
            .iter()
            .map(|region| self.device.dma.read_memory(region.base(), region.length()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DeviceStateSnapshot {
            mappings,
            bytes,
            memory_generation: self.device.dma.generation(),
            resources: self.device.execution.resource_config(),
            trace: self.device.execution.trace_events(),
            runner: super::state::snapshot(self),
            runner_generation: self.generation,
            partitions: self.reset_partition_snapshot(),
            irq: self.device.execution.irq_snapshot(),
            reset_runner_calls: self.reset_runner_calls(),
        })
    }
}

impl AsyncTestSupport for AsyncDeviceHandle {
    async fn state_snapshot(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
        CoreTestSupport::state_snapshot(&*self.shared.state.lock().await)
    }
}

impl SyncTestSupport for SyncDevice {
    fn state_snapshot(&self) -> Result<DeviceStateSnapshot, PdmaMemoryError> {
        CoreTestSupport::state_snapshot(&**self)
    }
}
