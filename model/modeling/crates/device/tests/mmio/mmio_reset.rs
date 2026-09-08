#![allow(missing_docs)]

use pdma_model_device::ModelError;

mod support;

use pdma_model_contract::abi::mmio::MmioResult;
use pdma_model_contract::semantic::{
    Command, CommandId, CopyRequest, EventRouting, QueueConfig, QueueId, SoftwareOwnerId,
};
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};
use pdma_model_device::{AsyncDevice, PdmaControlError};
use support::{TestResult, raw_reset, read_u32, read_u64, write_u64};

const fn config() -> QueueConfig {
    QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}

#[tokio::test(flavor = "current_thread")]
async fn typed_reset_clears_resettable_state_and_preserves_persistent_partition() -> TestResult {
    // Given: completed work, pending record/IRQ state, persistent DMA, trace, and staged banks.
    let (device, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    handle.enable_tracing().await;
    let queue = handle.create_queue(config()).await?;
    handle
        .enable_irq_source(queue.completion_irq_source())
        .await?;
    handle
        .unmask_irq_source(queue.completion_irq_source())
        .await?;
    let source = handle.allocate_buffer(1).await?;
    let destination = handle.allocate_buffer(1).await?;
    handle.write_buffer(source, 0, &[0x6d]).await?;
    handle
        .submit_command(
            queue,
            Command::new(
                CommandId::new(0),
                pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
                    source,
                    destination,
                    1,
                )),
            ),
        )
        .await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(0).await;
    let memory_generation = handle.memory_generation().await;
    let trace = handle.trace_events().await;
    let snapshot = handle.snapshot().await;
    write_u64(&handle, 0x010, 9).await?;
    write_u64(&handle, 0x108, 7).await?;
    write_u64(&handle, 0x208, 5).await?;

    // When: the typed API invokes the shared Task 9 reset seam for the current epoch.
    let epoch = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;

    // Then: all register banks and resettable state clear while persistent state survives.
    assert_eq!(epoch, DeviceEpoch::new(1));
    assert_eq!(read_u64(&handle, 0x010).await?, 0);
    assert_eq!(read_u64(&handle, 0x108).await?, 0);
    assert_eq!(read_u64(&handle, 0x208).await?, 0);
    assert_eq!(read_u32(&handle, 0x168).await?, 0);
    assert_eq!(read_u32(&handle, 0x224).await?, 0);
    assert_eq!(handle.memory_generation().await, memory_generation);
    assert_eq!(handle.read_buffer(destination, 0, 1).await?, [0x6d]);
    assert_eq!(handle.trace_events().await, trace);
    assert_eq!(handle.snapshot().await.steps, snapshot.steps);
    assert!(matches!(
        handle.ack_irq_source(queue.completion_irq_source()).await,
        Err(ModelError::StaleDeviceEpoch { .. })
    ));
    let fresh = handle.create_queue(config()).await?;
    assert_eq!(fresh.epoch(), DeviceEpoch::new(1));
    assert_eq!(fresh.queue_id(), QueueId::new(0));
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn stale_typed_reset_rejects_before_any_mutation() -> TestResult {
    // Given: epoch one with committed DMA and new nonzero queue/IRQ staging.
    let (device, _worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let handle = device.handle();
    let buffer = handle.allocate_buffer(1).await?;
    handle.write_buffer(buffer, 0, &[0x3c]).await?;
    let epoch = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    write_u64(&handle, 0x108, 7).await?;
    write_u64(&handle, 0x208, 5).await?;

    // When: a stale request is sent through the typed API that shares the reset seam.
    let result = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await;

    // Then: epoch, staged banks, and persistent DMA remain unchanged.
    assert_eq!(epoch, DeviceEpoch::new(1));
    assert_eq!(
        result,
        Err(PdmaControlError::StaleEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    );
    assert_eq!(read_u64(&handle, 0x108).await?, 7);
    assert_eq!(read_u64(&handle, 0x208).await?, 5);
    assert_eq!(handle.read_buffer(buffer, 0, 1).await?, [0x3c]);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn global_reset_dispatch_returns_post_reset_receipt_and_preserves_dma() -> TestResult {
    // Given: a published global response, staged queue/IRQ banks, and committed DMA.
    let (device, _worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let handle = device.handle();
    let buffer = handle.allocate_buffer(1).await?;
    handle.write_buffer(buffer, 0, &[0xa5]).await?;
    support::write_u32(&handle, 0x020, 3).await?;
    write_u64(&handle, 0x108, 7).await?;
    write_u64(&handle, 0x208, 5).await?;

    // When: raw global RESET executes through the shared whole-device reset core.
    raw_reset(&handle, 0).await?;

    // Then: it posts the new epoch, clears resettable banks, and preserves committed DMA.
    assert_eq!(read_u32(&handle, 0x024).await?, MmioResult::Ok.code());
    assert_eq!(
        (
            read_u64(&handle, 0x028).await?,
            read_u64(&handle, 0x030).await?,
            read_u64(&handle, 0x038).await?
        ),
        (0, 0, 0)
    );
    assert_eq!(read_u64(&handle, 0x050).await?, 1);
    assert_eq!(read_u64(&handle, 0x108).await?, 0);
    assert_eq!(read_u64(&handle, 0x208).await?, 0);
    assert_eq!(handle.read_buffer(buffer, 0, 1).await?, [0xa5]);
    Ok(())
}
