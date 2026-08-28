#![allow(missing_docs)]

use pdma_model_device::ModelError;

use pdma_model_contract::semantic::{
    Command, CommandId, CopyRequest, DeviceEpoch, EpochIrqSource, EventRouting, IrqRoute,
    IrqSource, IrqVector, QueueActivation, QueueConfig, QueueControl, QueueId, QueueRingObjects,
    ResourceConfig, SoftwareOwnerId,
};
use pdma_model_contract::semantic::{EpochQueue, WholeDeviceReset};
use pdma_model_device::{AsyncStallReason, AsyncStepStatus, SyncDevice};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const fn config() -> QueueConfig {
    QueueConfig::new(2, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}

#[tokio::test]
async fn whole_device_reset_preserves_dma_and_allows_new_epoch_bootstrap() -> TestResult {
    // Given: one completed copy, retained trace history, and both queue frontends at epoch zero.
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    device.enable_tracing();
    let old_queue = device.create_queue(config())?;
    let old_bus_queue = device.create_dma_bus_queue(config())?;
    let source = device.allocate_buffer(4)?;
    let destination = device.allocate_buffer(4)?;
    device.write_buffer(source, 0, &[1, 3, 5, 7])?;
    device.submit_command(
        old_queue,
        Command::new(
            CommandId::new(0),
            pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
                source,
                destination,
                4,
            )),
        ),
    )?;
    device.ring_doorbell(old_queue)?;
    device.run_until_idle(1)?;
    let memory_generation = device.memory_generation();
    let trace_before = device.trace_events();

    // When: a whole-device reset advances the expected epoch.
    let epoch = device.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;

    // Then: DMA and instrumentation persist, old capabilities stale, and queue zero is reusable.
    assert_eq!(epoch, DeviceEpoch::new(1));
    assert_eq!(device.memory_generation(), memory_generation);
    assert_eq!(device.read_buffer(destination, 0, 4)?, [1, 3, 5, 7]);
    assert_eq!(device.trace_events(), trace_before);
    assert!(matches!(
        device.ring_doorbell(old_queue),
        Err(ModelError::StaleDeviceEpoch { .. })
    ));
    assert!(matches!(
        device.ring_doorbell(old_bus_queue),
        Err(ModelError::StaleDeviceEpoch { .. })
    ));
    let new_queue = device.create_queue(config())?;
    assert_eq!(new_queue.epoch(), epoch);
    assert_eq!(new_queue.queue_id(), QueueId::new(0));

    let (async_device, _worker) =
        pdma_model_device::AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
            std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
        ));
    let handle = async_device.handle();
    let old_rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let old_activation = activation(&old_rings, DeviceEpoch::initial())?;
    let old_status = handle
        .activate_queue(old_rings.clone(), old_activation)
        .await?;
    let old_dma_generation = handle.memory_generation().await;
    let snapshot = handle.snapshot().await;
    assert_eq!(
        handle
            .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
            .await?,
        DeviceEpoch::new(1)
    );
    assert!(handle.snapshot().await.generation() > snapshot.generation());
    assert_eq!(handle.memory_generation().await, old_dma_generation);
    assert_eq!(
        handle
            .control_queue(QueueControl::ReadStatus(old_status.queue()))
            .await,
        Err(pdma_model_device::PdmaControlError::StaleEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    );
    let new_rings = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let new_status = handle
        .activate_queue(
            new_rings.clone(),
            activation(&new_rings, DeviceEpoch::new(1))?,
        )
        .await?;
    assert_eq!(
        new_status.queue(),
        EpochQueue::new(DeviceEpoch::new(1), QueueId::new(0))
    );
    Ok(())
}

#[test]
fn reset_discards_stalled_and_pending_retirement_without_late_publication() -> TestResult {
    // Given: one resource-stalled command and one completion pending retirement.
    let mut stalled = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_read_slots(0),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let stalled_queue = stalled.create_queue(config())?;
    let source = stalled.allocate_buffer(1)?;
    let destination = stalled.allocate_buffer(1)?;
    stalled.write_buffer(source, 0, &[0x5a])?;
    submit_copy(&mut stalled, stalled_queue, source, destination, 0)?;
    let status = stalled.run_until_blocked(1)?;
    assert_eq!(
        status.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::ExecutionResourcePressure)
    );

    let mut retiring = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let retiring_queue = retiring.create_queue(config())?;
    let retiring_source = retiring.allocate_buffer(1)?;
    let first_destination = retiring.allocate_buffer(1)?;
    let second_destination = retiring.allocate_buffer(1)?;
    retiring.write_buffer(retiring_source, 0, &[0xa5])?;
    submit_copy(
        &mut retiring,
        retiring_queue,
        retiring_source,
        first_destination,
        0,
    )?;
    submit_copy(
        &mut retiring,
        retiring_queue,
        retiring_source,
        second_destination,
        1,
    )?;
    retiring.run_until_blocked(2)?;

    // When: each whole device resets while work is retained but unpublished.
    stalled.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;
    retiring.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;

    // Then: no discarded work executes or publishes, while prior memory effects persist.
    assert_eq!(stalled.read_buffer(destination, 0, 1)?, [0]);
    assert_eq!(retiring.read_buffer(first_destination, 0, 1)?, [0xa5]);
    assert_eq!(retiring.read_buffer(second_destination, 0, 1)?, [0xa5]);
    assert_eq!(stalled.run_until_idle(1)?.steps, 0);
    assert_eq!(retiring.run_until_idle(1)?.steps, 0);
    assert!(stalled.irq_snapshot().is_none());
    assert!(retiring.irq_snapshot().is_none());
    Ok(())
}

#[test]
fn old_notification_and_epoch_overflow_mutate_nothing() -> TestResult {
    // Given: one host-owned message notification and a fully exhausted epoch value.
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let queue = device.create_queue(config())?;
    let source = queue.completion_irq_source();
    device.configure_irq_route(source, IrqRoute::Message(IrqVector::new(7)))?;
    device.enable_irq_source(source)?;
    device.unmask_irq_source(source)?;
    let input = device.allocate_buffer(1)?;
    let output = device.allocate_buffer(1)?;
    device.write_buffer(input, 0, &[9])?;
    submit_copy(&mut device, queue, input, output, 0)?;
    device.run_until_idle(1)?;
    let notification = device
        .wait_irq_notification()
        .ok_or("message notification must be host-owned")?;
    device.reset(WholeDeviceReset::new(DeviceEpoch::initial()))?;
    let bytes_before = device.read_buffer(output, 0, 1)?;
    let trace_before = device.trace_events();

    // When: the old source is serviced and checked epoch arithmetic overflows.
    let stale_ack = device.ack_irq_source(notification.source_capability());
    let overflow = DeviceEpoch::new(u64::MAX).next();

    // Then: neither rejected operation mutates bytes, trace, IRQ, or queue identity.
    assert!(matches!(
        stale_ack,
        Err(ModelError::StaleDeviceEpoch { .. })
    ));
    assert!(overflow.is_err());
    assert_eq!(device.read_buffer(output, 0, 1)?, bytes_before);
    assert_eq!(device.trace_events(), trace_before);
    assert!(device.irq_snapshot().is_none());
    let fresh = device.create_queue(config())?;
    assert_eq!(fresh.queue_id(), QueueId::new(0));
    assert_eq!(
        fresh.completion_irq_source(),
        EpochIrqSource::new(
            DeviceEpoch::new(1),
            IrqSource::Completion(fresh.completion_ring_id())
        )
    );
    Ok(())
}

fn submit_copy(
    device: &mut SyncDevice,
    queue: pdma_model_contract::semantic::QueueConfigHandle,
    source: pdma_model_contract::semantic::BufferId,
    destination: pdma_model_contract::semantic::BufferId,
    command: usize,
) -> TestResult {
    device.submit_command(
        queue,
        Command::new(
            CommandId::new(command),
            pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
                source,
                destination,
                1,
            )),
        ),
    )?;
    device.ring_doorbell(queue)?;
    Ok(())
}

fn activation(
    stage: &pdma_model_device::PdmaMemoryStage,
    epoch: DeviceEpoch,
) -> Result<QueueActivation, pdma_model_contract::abi::AbiControlError> {
    let [submission, completion, event] = stage.regions() else {
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual: u32::MAX,
            },
        );
    };
    Ok(QueueActivation::new(
        epoch,
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingObjects::new(submission.object(), completion.object(), event.object()),
    ))
}
