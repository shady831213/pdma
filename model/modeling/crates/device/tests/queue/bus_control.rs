use super::hardware::{self, TestResult, configure, read, write};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{GlobalRegisters, QueueRegisters, queue_offset};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};
mod activation;
mod failure;

fn new_handle() -> (
    AsyncDeviceHandle,
    hardware_interface_runtime::SharedMemoryAllocator,
) {
    let memory = hardware::allocator();
    let (device, _worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    (device.handle(), memory)
}

#[tokio::test]
async fn producer_publication_preserves_dma_and_requires_capture() -> TestResult {
    // Given: committed indirect and descriptor bytes exist.
    let (handle, memory) = new_handle();
    let [submission, _, _] = hardware::activate(&handle, &memory, 0, [2; 3]).await?;
    let indirect = hardware::allocate(&memory, 16)?;
    handle.write_memory(indirect.base(), &[0x31; 16]).await?;
    handle.write_memory(submission, &[0x7a; 64]).await?;
    let endpoint = handle.register_endpoint();
    // When: software publishes the SQ producer.
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?,
        1,
    )
    .await?;
    // Then: capture and consumption remain zero while DMA bytes persist.
    assert_eq!(
        read(
            &endpoint,
            queue_offset(0, QueueRegisters::SQ_CAPTURED.offset())?
        )
        .await?,
        0
    );
    assert_eq!(
        read(
            &endpoint,
            queue_offset(0, QueueRegisters::SQ_CONSUMER.offset())?
        )
        .await?,
        0
    );
    assert_eq!(handle.read_memory(indirect.base(), 16).await?, [0x31; 16]);
    assert_eq!(handle.read_memory(submission, 64).await?, [0x7a; 64]);
    Ok(())
}

#[tokio::test]
async fn invalid_cursor_and_device_owned_writes_leave_state_unchanged() -> TestResult {
    // Given: an active depth-two queue starts empty.
    let (handle, memory) = new_handle();
    hardware::activate(&handle, &memory, 0, [2; 3]).await?;
    let endpoint = handle.register_endpoint();
    // When: software attempts invalid cursor and device-owned writes.
    for (register, value) in [
        (QueueRegisters::SQ_PRODUCER.offset(), 3),
        (QueueRegisters::DOORBELL.offset(), 1),
        (QueueRegisters::CQ_CONSUMER.offset(), 1),
        (QueueRegisters::EQ_CONSUMER.offset(), 1),
        (QueueRegisters::SQ_CONSUMER.offset(), 1),
        (QueueRegisters::CQ_PRODUCER.offset(), 1),
        (QueueRegisters::EQ_PRODUCER.offset(), 1),
    ] {
        let before = handle.snapshot().await;
        assert!(
            write(&endpoint, queue_offset(0, register)?, value)
                .await
                .is_err()
        );
        assert_eq!(handle.snapshot().await, before);
    }
    // Then: all readable cursors retain their initial values.
    for offset in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
    ] {
        assert_eq!(read(&endpoint, queue_offset(0, offset)?).await?, 0);
    }
    Ok(())
}

#[tokio::test]
async fn repeated_publication_is_idempotent_and_regression_rejects() -> TestResult {
    // Given: one descriptor producer has been captured.
    let (handle, memory) = new_handle();
    hardware::activate(&handle, &memory, 0, [2; 3]).await?;
    let endpoint = handle.register_endpoint();
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?,
        1,
    )
    .await?;
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::DOORBELL.offset())?,
        1,
    )
    .await?;
    // When: software repeats current cursors and attempts regression.
    let before = handle.snapshot().await;
    for (offset, value) in [
        (QueueRegisters::SQ_PRODUCER.offset(), 1),
        (QueueRegisters::DOORBELL.offset(), 1),
        (QueueRegisters::CQ_CONSUMER.offset(), 0),
        (QueueRegisters::EQ_CONSUMER.offset(), 0),
    ] {
        write(&endpoint, queue_offset(0, offset)?, value).await?;
    }
    // Then: idempotent writes preserve generation and regressions reject.
    assert_eq!(handle.snapshot().await, before);
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?,
            0
        )
        .await
        .is_err()
    );
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::DOORBELL.offset())?,
            0
        )
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn reset_reuses_mappings_only_through_a_fresh_endpoint() -> TestResult {
    // Given: activated rings contain committed bytes.
    let (handle, memory) = new_handle();
    let rings = hardware::activate(&handle, &memory, 0, [2; 3]).await?;
    handle.write_memory(rings[0], &[0x44; 64]).await?;
    let endpoint = handle.register_endpoint();
    // When: reset invalidates the old endpoint and software binds the discovered epoch.
    write(
        &endpoint,
        u32::try_from(GlobalRegisters::RESET.offset())?,
        1,
    )
    .await?;
    assert!(
        write(
            &endpoint,
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            1
        )
        .await
        .is_err()
    );
    let epoch = read(&endpoint, u32::try_from(GlobalRegisters::EPOCH.offset())?).await?;
    assert_eq!(epoch, 1);
    let fresh = endpoint.at_epoch(DeviceEpoch::new(epoch));
    configure(&fresh, 0, rings.map(|base| (base, 2))).await?;
    write(&fresh, queue_offset(0, QueueRegisters::ENABLE.offset())?, 1).await?;
    // Then: fresh activation resets cursors and preserves DMA mappings and bytes.
    assert_eq!(handle.read_memory(rings[0], 64).await?, [0x44; 64]);
    for offset in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
    ] {
        assert_eq!(read(&fresh, queue_offset(0, offset)?).await?, 0);
    }
    Ok(())
}

#[tokio::test]
async fn dropped_enable_future_changes_no_state() -> TestResult {
    // Given: committed rings have been configured but not enabled.
    let (handle, memory) = new_handle();
    let rings = hardware::rings(&memory, [2; 3])?;
    let endpoint = handle.register_endpoint();
    configure(&endpoint, 0, rings.map(|base| (base, 2))).await?;
    let before = handle.snapshot().await;
    let enable = 1_u32.to_le_bytes();
    // When: an enable future is dropped without polling.
    drop(endpoint.write(queue_offset(0, QueueRegisters::ENABLE.offset())?, &enable));
    // Then: the queue remains inactive without a worker generation change.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        read(&endpoint, queue_offset(0, QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    Ok(())
}
