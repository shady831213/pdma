#![allow(missing_docs)]

use super::mmio_irq::support::{enable, i};
use super::mmio_queue::support::*;
use pdma_model_contract::abi::registers::{GlobalRegisters, IrqSourceRegisters, QueueRegisters};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};
use pdma_model_device::{MmioRegisterError, ModelError};

#[tokio::test(flavor = "current_thread")]
async fn reset_clears_resettable_state_and_preserves_persistent_partition() -> TestResult {
    // Given: completed work with unread completion/IRQ and persistent DMA and trace history.
    let (device, worker) = device(ResourceConfig::default());
    let handle = device.handle();
    handle.enable_tracing().await;
    let rings = create_queue(&handle, 2).await?;
    let endpoint = handle.register_endpoint();
    enable(&endpoint).await?;
    let destination = submit_copy(&handle, rings[0].base(), 0).await?;
    write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 1).await?;
    let worker = tokio::spawn(worker.run());
    device.wait_for_progress_after(0).await;
    assert!(handle.irq_notification_ready().await);
    let memory_generation = handle.memory_generation().await;
    let trace = handle.trace_events().await;
    let before = handle.snapshot().await;
    // When: software writes the whole-device RESET register.
    write_u32(
        &endpoint,
        u32::try_from(GlobalRegisters::RESET.offset())?,
        1,
    )
    .await?;
    // Then: live registers clear while committed mappings, bytes and accounting survive.
    assert_eq!(
        read_u64(&endpoint, u32::try_from(GlobalRegisters::EPOCH.offset())?).await?,
        1
    );
    assert_eq!(
        read_u32(
            &endpoint,
            u32::try_from(GlobalRegisters::RUN_STATE.offset())?
        )
        .await?,
        0
    );
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    assert_eq!(
        read_u64(&endpoint, q(QueueRegisters::SQ_BASE.offset())?).await?,
        0
    );
    assert_eq!(
        read_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    assert!(!handle.irq_notification_ready().await);
    assert_eq!(handle.memory_generation().await, memory_generation);
    assert_eq!(
        handle.read_memory(destination.base(), 4).await?,
        [1, 2, 3, 4]
    );
    assert_eq!(handle.trace_events().await, trace);
    assert_eq!(handle.snapshot().await.steps, before.steps);
    assert!(matches!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await,
        Err(ModelError::StaleDeviceEpoch { .. })
    ));
    let fresh = endpoint.at_epoch(DeviceEpoch::new(1));
    program_activation(&fresh, &rings, 2).await?;
    write_u32(&fresh, q(QueueRegisters::ENABLE.offset())?, 1).await?;
    assert_eq!(cursors(&fresh).await?, [0; 7]);
    assert_eq!(
        read_u64(&fresh, i(IrqSourceRegisters::PENDING_MESSAGES.offset())?).await?,
        0
    );
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test]
async fn stale_reset_rejects_before_mutating_current_configuration() -> TestResult {
    // Given: epoch one with committed DMA and new queue and IRQ configuration.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let buffer = handle.allocate_memory_region(1).await?;
    handle.write_memory(buffer.base(), &[0x3c]).await?;
    let stale = handle.register_endpoint();
    let reset = u32::try_from(GlobalRegisters::RESET.offset())?;
    write_u32(&stale, reset, 1).await?;
    let fresh = stale.at_epoch(DeviceEpoch::new(1));
    write_u64(&fresh, q(QueueRegisters::SQ_BASE.offset())?, 0x1000).await?;
    write_u64(&fresh, i(IrqSourceRegisters::ROUTE_VECTOR.offset())?, 5).await?;
    let before = handle.snapshot().await;
    // When: a prior-epoch endpoint repeats reset.
    let result = write_u32(&stale, reset, 1).await;
    // Then: the typed stale failure preserves current registers, bytes and lifecycle.
    assert_eq!(
        result,
        Err(MmioRegisterError::StaleEpoch {
            expected: DeviceEpoch::new(1),
            actual: DeviceEpoch::initial(),
        })
    );
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        read_u64(&fresh, q(QueueRegisters::SQ_BASE.offset())?).await?,
        0x1000
    );
    assert_eq!(
        read_u64(&fresh, i(IrqSourceRegisters::ROUTE_VECTOR.offset())?).await?,
        5
    );
    assert_eq!(handle.read_memory(buffer.base(), 1).await?, [0x3c]);
    Ok(())
}
