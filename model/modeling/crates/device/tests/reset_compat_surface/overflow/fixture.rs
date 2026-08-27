use std::sync::Arc;

use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};
use pdma_model_device::DeviceCore;
use pdma_model_device::compat::SyncDevice;
use pdma_model_device::test_support::{
    DeviceStateSnapshot, device_core_at_epoch_with_mmio_write, state_snapshot,
};

use super::super::support::{RecordingObserver, TestResult, queue_config, submit_copy};

pub(super) struct Fixture {
    pub(super) device: SyncDevice,
    observer: Arc<RecordingObserver>,
    default_registers: Vec<Vec<u8>>,
}

pub(super) fn populated() -> TestResult<Fixture> {
    let memory = Arc::new(SharedMemory::new());
    let observer = Arc::new(RecordingObserver::default());
    let defaults = state_snapshot(&DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ))?
    .partitions
    .registers;
    let core = device_core_at_epoch_with_mmio_write(
        Arc::clone(&memory),
        ResourceConfig::default(),
        observer.clone(),
        DeviceEpoch::new(u64::MAX),
        0x010,
        &0xfeed_face_cafe_babe_u64.to_le_bytes(),
    )?;
    let mut device = SyncDevice::from_core(core);
    device.enable_tracing();
    let queue = device.create_queue(queue_config())?;
    let source = device.allocate_buffer(1)?;
    let first = device.allocate_buffer(1)?;
    let second = device.allocate_buffer(1)?;
    device.write_buffer(source, 0, &[0xa5])?;
    submit_copy(&mut device, queue, source, first, 0)?;
    submit_copy(&mut device, queue, source, second, 1)?;
    let irq = queue.completion_irq_source();
    device.enable_irq_source(irq)?;
    device.unmask_irq_source(irq)?;
    let summary = device.run_until_blocked_typed(2)?;
    assert_eq!(summary.steps, 1);
    assert_eq!(summary.stalls, 1);
    Ok(Fixture {
        device,
        observer,
        default_registers: defaults,
    })
}

impl Fixture {
    pub(super) fn assert_preconditions(&self, state: &DeviceStateSnapshot) {
        assert_eq!(state.mappings.len(), 3);
        assert_eq!(state.bytes.first(), Some(&vec![0xa5]));
        assert_eq!(state.memory_generation, 3);
        assert_eq!(state.resources, ResourceConfig::default());
        assert!(!state.trace.is_empty());
        assert_eq!(state.runner.steps, 1);
        assert_eq!(state.partitions.queues, 1);
        assert_eq!(state.partitions.results, 1);
        assert_eq!(state.partitions.work, 0);
        assert_eq!(state.partitions.retirement, 1);
        assert_eq!(state.partitions.irq_sources, 1);
        assert_ne!(state.partitions.registers, self.default_registers);
        assert_eq!(state.partitions.backend_epoch, DeviceEpoch::new(u64::MAX));
        assert_eq!(state.partitions.control_epoch, DeviceEpoch::new(u64::MAX));
        assert!(state.irq.is_some());
        assert!(self.observer.snapshot().is_empty());
        assert_eq!(state.reset_runner_calls, 0);
    }

    pub(super) fn assert_unchanged(
        &self,
        before: &DeviceStateSnapshot,
        after: &DeviceStateSnapshot,
    ) -> usize {
        assert_eq!(after.mappings, before.mappings);
        assert_eq!(after.bytes, before.bytes);
        assert_eq!(after.memory_generation, before.memory_generation);
        assert_eq!(after.resources, before.resources);
        assert_eq!(after.trace, before.trace);
        assert_eq!(after.runner, before.runner);
        assert_eq!(after.runner_generation, before.runner_generation);
        assert_eq!(after.partitions.queues, before.partitions.queues);
        assert_eq!(after.partitions.results, before.partitions.results);
        assert_eq!(after.partitions.work, before.partitions.work);
        assert_eq!(after.partitions.retirement, before.partitions.retirement);
        assert_eq!(after.partitions.irq_sources, before.partitions.irq_sources);
        assert_eq!(after.partitions.registers, before.partitions.registers);
        assert_eq!(
            after.partitions.backend_epoch,
            before.partitions.backend_epoch
        );
        assert_eq!(
            after.partitions.control_epoch,
            before.partitions.control_epoch
        );
        assert_eq!(after.irq, before.irq);
        assert_eq!(self.observer.snapshot(), Vec::new());
        assert_eq!(after.reset_runner_calls, before.reset_runner_calls);
        0
    }
}

pub(super) fn require_error<T, E>(result: Result<T, E>) -> TestResult<E> {
    match result {
        Ok(_) => Err("maximum epoch reset unexpectedly committed".into()),
        Err(error) => Ok(error),
    }
}
