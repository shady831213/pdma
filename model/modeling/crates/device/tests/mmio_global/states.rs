use super::support::{TestResult, response, result, write_u32};
use pdma_model_contract::semantic::{
    Command, CommandId, DeviceEpoch, EpochQueue, QueueActivation, QueueControl, QueueId,
    QueueRingObjects, ResourceConfig, WholeDeviceReset,
    desc::CopyRequest,
    ids::BufferId,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};

pub async fn assert_run_state_matrix() -> TestResult {
    run().await
}

async fn run() -> TestResult {
    assert_current_epoch_is_reported().await?;
    assert_reads_preserve_device_observables().await?;
    assert_unstarted_state(ResourceConfig::default(), None, 0).await?;
    assert_unstarted_state(ResourceConfig::default(), Some(valid_command), 1).await?;
    assert_unstarted_state(
        ResourceConfig::default().with_read_slots(0),
        Some(valid_command),
        2,
    )
    .await?;
    assert_worker_pressure(
        ResourceConfig::default().with_completion_publication_slots(0),
        valid_command,
    )
    .await?;
    assert_worker_pressure(
        ResourceConfig::default().with_event_publication_slots(0),
        faulting_command,
    )
    .await?;
    Ok(())
}

async fn assert_current_epoch_is_reported() -> TestResult {
    // Given: a device advanced through the existing typed whole-device reset.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let epoch = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;

    // When: status is read through the raw global command.
    write_u32(&handle, 0x020, 3).await?;

    // Then: the response contains the complete current epoch rather than reset value zero.
    assert_eq!(epoch, DeviceEpoch::new(1));
    assert_eq!(response(&handle).await?, [0, 0, 0, 1, 0, 0, 0, 0, 0]);
    Ok(())
}

async fn assert_unstarted_state(
    config: ResourceConfig,
    command: Option<fn(BufferId, BufferId) -> Command>,
    expected: u64,
) -> TestResult {
    // Given: an unstarted worker with either no work, runnable work, or constrained work.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        config,
    ));
    let handle = device.handle();
    let work = if let Some(build) = command {
        let (queue, source, destination) = fixture(&handle).await?;
        handle
            .submit_command(queue, build(source, destination))
            .await?;
        handle.ring_doorbell(queue).await?;
        Some((queue, destination))
    } else {
        None
    };
    let before = handle.snapshot().await;
    let dma_before = if let Some((_, destination)) = work {
        Some(handle.read_buffer(destination, 0, 4).await?)
    } else {
        None
    };
    let irq_before = handle.irq_snapshot().await;

    // When: READ_STATUS is issued repeatedly without running a worker step.
    for _ in 0..2 {
        write_u32(&handle, 0x020, 3).await?;
        assert_eq!(result(&handle).await?, [1, 0, 0, 0]);
        assert_eq!(response(&handle).await?, [0, expected, 0, 0, 0, 0, 0, 0, 0]);
    }

    // Then: status reads leave runner generation and all non-MMIO work state untouched.
    assert_eq!(handle.snapshot().await, before);
    if let Some((_, destination)) = work {
        assert_eq!(
            Some(handle.read_buffer(destination, 0, 4).await?),
            dma_before
        );
    }
    assert_eq!(handle.irq_snapshot().await, irq_before);
    Ok(())
}

async fn assert_reads_preserve_device_observables() -> TestResult {
    // Given: committed DMA, one active queue, and a stable IRQ snapshot.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let handle = device.handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [submission, completion, event] = stage.regions() else {
        return Err(std::io::Error::other("three ring regions required").into());
    };
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));
    let activation = QueueActivation::new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
        QueueRingObjects::new(submission.object(), completion.object(), event.object()),
    );
    handle.activate_queue(stage, activation).await?;
    let before_snapshot = handle.snapshot().await;
    let before_generation = handle.memory_generation().await;
    let before_queue = handle
        .control_queue(QueueControl::ReadStatus(queue))
        .await?;
    let before_irq = handle.irq_snapshot().await;

    // When: status is read repeatedly through raw MMIO.
    for _ in 0..2 {
        write_u32(&handle, 0x020, 3).await?;
    }

    // Then: lifecycle, DMA, queue, and IRQ observables are unchanged.
    assert_eq!(handle.snapshot().await, before_snapshot);
    assert_eq!(handle.memory_generation().await, before_generation);
    assert_eq!(
        handle
            .control_queue(QueueControl::ReadStatus(queue))
            .await?,
        before_queue
    );
    assert_eq!(handle.irq_snapshot().await, before_irq);
    Ok(())
}

async fn assert_worker_pressure(
    config: ResourceConfig,
    build: fn(BufferId, BufferId) -> Command,
) -> TestResult {
    // Given: a worker that reaches a real publication-pressure stall.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        config,
    ));
    let handle = device.handle();
    let (queue, source, destination) = fixture(&handle).await?;
    handle
        .submit_command(queue, build(source, destination))
        .await?;
    handle.ring_doorbell(queue).await?;
    let worker = tokio::spawn(worker.run());
    let stalled = device.wait_for_stall_after(0).await;

    // When: READ_STATUS probes the already-blocked state without taking a worker step.
    write_u32(&handle, 0x020, 3).await?;

    // Then: current state is Blocked and the query leaves the stalled snapshot unchanged.
    assert_eq!(response(&handle).await?, [0, 2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(handle.snapshot().await, stalled);
    device.request_shutdown().await;
    let _summary = worker.await?;
    Ok(())
}

async fn fixture(
    handle: &AsyncDeviceHandle,
) -> TestResult<(
    pdma_model_contract::semantic::QueueConfigHandle,
    BufferId,
    BufferId,
)> {
    let queue = handle
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await?;
    let source = handle.allocate_buffer(4).await?;
    let destination = handle.allocate_buffer(4).await?;
    handle.write_buffer(source, 0, &[1, 2, 3, 4]).await?;
    Ok((queue, source, destination))
}

const fn valid_command(source: BufferId, destination: BufferId) -> Command {
    Command::new(
        CommandId::new(7),
        pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
            source,
            destination,
            4,
        )),
    )
}

const fn faulting_command(_source: BufferId, destination: BufferId) -> Command {
    Command::new(
        CommandId::new(9),
        pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
            BufferId::new(99),
            destination,
            4,
        )),
    )
}
