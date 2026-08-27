use std::error::Error;
use std::sync::Arc;

use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory};
use pdma_model_contract::semantic::{
    CommandId, CompletionRingId, CopyCommand, CopyRequest, DeviceEpoch, EpochIrqSource,
    EventRouting, ExternalCommand, IrqRoute, IrqSource, IrqVector, QueueConfig, QueueConfigHandle,
    QueueDirectoryEntry, QueueId, ResourceConfig, SoftwareOwnerId, WholeDeviceReset,
};
use pdma_model_device::test_support::{DeviceStateSnapshot, sync_state_snapshot};
use pdma_model_device::{ModelError, PdmaRunError, SyncDevice};

use super::{TestResult, require_error};

#[test]
fn model_fate_rows_have_literal_leaf_sources() -> TestResult {
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    ));
    assert_leaf(
        &mut device,
        |core| core.create_queue(config(0)),
        ModelError::ZeroCapacity,
    )?;
    let queue = device.create_queue(config(1))?;
    let source = device.allocate_buffer(1)?;
    let destination = device.allocate_buffer(1)?;
    let command = ExternalCommand::Copy(CopyCommand::new(
        CommandId::new(0),
        queue.queue_id(),
        queue.completion_ring_id(),
        CopyRequest::new(source, destination, 1),
    ));
    device.submit_command(queue, command.clone())?;
    assert_leaf(
        &mut device,
        |core| core.submit_command(queue, command),
        ModelError::RingFull,
    )?;
    assert_leaf(
        &mut device,
        |core| core.read_buffer(pdma_model_contract::semantic::BufferId::new(99), 0, 1),
        ModelError::UnknownBuffer(pdma_model_contract::semantic::BufferId::new(99)),
    )?;
    assert_leaf(
        &mut device,
        |core| core.ring_doorbell(fake_queue()),
        ModelError::UnknownQueue(QueueId::new(99)),
    )?;
    assert_leaf(
        &mut device,
        |core| core.poll_completion(fake_completion_queue()),
        ModelError::UnknownCompletionRing(CompletionRingId::new(99)),
    )?;
    assert_leaf(
        &mut device,
        |core| {
            core.enable_irq_source(EpochIrqSource::new(
                DeviceEpoch::initial(),
                IrqSource::Completion(CompletionRingId::new(99)),
            ))
        },
        ModelError::UnknownIrqSource(IrqSource::Completion(CompletionRingId::new(99))),
    )?;
    let irq = queue.completion_irq_source();
    device.enable_irq_source(irq)?;
    assert_leaf(
        &mut device,
        |core| core.configure_irq_route(irq, IrqRoute::Message(IrqVector::new(1))),
        ModelError::IrqSourceActive(irq.source()),
    )?;
    assert_leaf(
        &mut device,
        |core| core.read_buffer(source, usize::MAX, 1),
        ModelError::RangeOverflow,
    )?;
    assert_leaf(
        &mut device,
        |core| core.read_buffer(source, 1, 1),
        ModelError::OutOfBounds {
            buffer: source,
            len: 1,
            end: 2,
        },
    )?;
    assert_leaf(
        &mut device,
        |core| core.allocate_buffer(usize::MAX),
        ModelError::DmaAllocationExhausted,
    )?;
    assert_run_and_stale(&mut device, queue)
}

fn assert_run_and_stale(device: &mut SyncDevice, queue: QueueConfigHandle) -> TestResult {
    device.ring_doorbell(queue)?;
    let before_run = sync_state_snapshot(device)?;
    let run = require_error(device.run_until_idle_typed(0))?;
    assert_eq!(
        run,
        PdmaRunError::Semantic(ModelError::StepBudgetExhausted { max_steps: 0 })
    );
    assert_eq!(
        run.source().and_then(|source| source.downcast_ref()),
        Some(&ModelError::StepBudgetExhausted { max_steps: 0 })
    );
    assert_unchanged(device, &before_run)?;
    device.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;
    let before_stale = sync_state_snapshot(device)?;
    assert_leaf(
        device,
        |core| core.ring_doorbell(queue),
        ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        },
    )?;
    assert_unchanged(device, &before_stale)
}

fn assert_leaf<T>(
    device: &mut SyncDevice,
    operation: impl FnOnce(&mut SyncDevice) -> Result<T, ModelError>,
    expected: ModelError,
) -> TestResult {
    let before = sync_state_snapshot(device)?;
    let error = require_error(operation(device))?;
    assert_eq!(error, expected);
    assert!(error.source().is_none());
    assert_unchanged(device, &before)
}

fn assert_unchanged(device: &SyncDevice, before: &DeviceStateSnapshot) -> TestResult {
    let after = sync_state_snapshot(device)?;
    assert_eq!(after.mappings, before.mappings);
    assert_eq!(after.bytes, before.bytes);
    assert_eq!(after.memory_generation, before.memory_generation);
    assert_eq!(after.resources, before.resources);
    assert_eq!(after.trace, before.trace);
    assert_eq!(after.runner, before.runner);
    assert_eq!(after.runner_generation, before.runner_generation);
    assert_eq!(after.partitions, before.partitions);
    assert_eq!(after.irq, before.irq);
    assert_eq!(after.reset_runner_calls, before.reset_runner_calls);
    Ok(())
}

const fn config(depth: usize) -> QueueConfig {
    QueueConfig::new(depth, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}

const fn fake_queue() -> QueueConfigHandle {
    QueueConfigHandle::new(
        DeviceEpoch::initial(),
        QueueDirectoryEntry::new(QueueId::new(99), CompletionRingId::new(99), config(1)),
    )
}

const fn fake_completion_queue() -> QueueConfigHandle {
    QueueConfigHandle::new(
        DeviceEpoch::initial(),
        QueueDirectoryEntry::new(QueueId::new(0), CompletionRingId::new(99), config(1)),
    )
}
