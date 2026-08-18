#![allow(missing_docs)]

#[path = "mmio_irq/support.rs"]
pub mod support;

use pdma_model_contract::abi::mmio::IrqSourceStatus;
use pdma_model_device::AsyncDevice;
use support::{
    RESULT, TestResult, create_queue, read_u32, read_u64, response, select_completion, submit_copy,
    write_u32, write_u64,
};

#[tokio::test(flavor = "current_thread")]
async fn source_commands_return_complete_post_command_state() -> TestResult {
    // Given: one valid completion source with an untouched response bank.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let handle = device.handle();
    let queue = create_queue(&handle, 1).await?;
    select_completion(&handle, queue).await?;

    // When: all seven source commands run through the raw selector window.
    write_u32(&handle, 0x220, 7).await?;
    assert_eq!(response(&handle).await?, [0, 0, 0, 5, 0, 0, 0, 0, 0]);
    write_u32(&handle, 0x214, 1).await?;
    write_u64(&handle, 0x218, 33).await?;
    let commands_and_states = [(1, 5), (2, 7), (4, 7), (5, 3), (3, 1), (6, 1)];
    for (command, state) in commands_and_states {
        write_u32(&handle, 0x220, command).await?;
        let bank = response(&handle).await?;
        assert_eq!(read_u32(&handle, RESULT).await?, 1);
        assert_eq!(bank, [0, 0, 0, state, 1, 0, 33, 0, 0]);
    }

    // Then: the typed observation agrees and notification-only fields stayed zero.
    assert_eq!(
        handle
            .observe_irq_source(queue.completion_irq_source())
            .await?,
        IrqSourceStatus::try_from(1).map_err(|_| "assigned state")?
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn line_snapshot_claim_and_ack_are_non_consuming_until_records_drain() -> TestResult {
    // Given: one published completion on an enabled unmasked Line source.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let queue = create_queue(&handle, 1).await?;
    select_completion(&handle, queue).await?;
    write_u32(&handle, 0x220, 2).await?;
    write_u32(&handle, 0x220, 5).await?;
    submit_copy(&handle, queue, 0).await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(0).await;

    // When: readiness, two snapshots, a claim, and ACK-before-drain observe the cause.
    assert!(handle.irq_notification_ready().await);
    for command in [8, 8, 9, 6] {
        write_u32(&handle, 0x220, command).await?;
        assert_eq!(read_u32(&handle, RESULT).await?, 1);
        let bank = response(&handle).await?;
        if command == 6 {
            assert_eq!(bank, [0, 0, 0, 123, 0, 0, 0, 0, 0]);
        } else {
            assert_eq!(bank, [0, 0, 0, 123, 0, 1, 0, 0, 0]);
        }
    }

    // Then: only draining the record followed by ACK clears readiness and EMPTY zero-fills.
    handle.poll_completion(queue).await?.ok_or("completion")?;
    write_u32(&handle, 0x220, 6).await?;
    assert_eq!(response(&handle).await?, [0, 0, 0, 3, 0, 0, 0, 0, 0]);
    assert!(!handle.irq_notification_ready().await);
    write_u32(&handle, 0x220, 8).await?;
    assert_eq!(read_u32(&handle, RESULT).await?, 2);
    assert_eq!(response(&handle).await?, [0; 9]);
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn message_readiness_is_non_consuming_and_claim_consumes_exactly_one() -> TestResult {
    // Given: two completions on a Message source that was masked during publication.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let queue = create_queue(&handle, 2).await?;
    select_completion(&handle, queue).await?;
    write_u32(&handle, 0x214, 1).await?;
    write_u64(&handle, 0x218, 41).await?;
    write_u32(&handle, 0x220, 1).await?;
    write_u32(&handle, 0x220, 2).await?;
    submit_copy(&handle, queue, 0).await?;
    submit_copy(&handle, queue, 1).await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(1).await;

    // When: masked claim is EMPTY, unmasking makes repeated readiness checks true, then claims run.
    assert!(!handle.irq_notification_ready().await);
    write_u32(&handle, 0x220, 9).await?;
    assert_eq!(read_u32(&handle, RESULT).await?, 2);
    assert_eq!(response(&handle).await?, [0; 9]);
    write_u32(&handle, 0x220, 5).await?;
    assert!(handle.irq_notification_ready().await);
    assert!(handle.irq_notification_ready().await);
    write_u32(&handle, 0x220, 9).await?;
    assert_eq!(response(&handle).await?, [0, 0, 0, 91, 1, 2, 41, 0, 1]);
    write_u32(&handle, 0x220, 9).await?;
    assert_eq!(response(&handle).await?, [0, 0, 0, 75, 1, 2, 41, 1, 0]);

    // Then: a race-losing third claim is EMPTY and shutdown does not manufacture a notification.
    assert!(!handle.irq_notification_ready().await);
    write_u32(&handle, 0x220, 9).await?;
    assert_eq!(read_u32(&handle, RESULT).await?, 2);
    assert_eq!(response(&handle).await?, [0; 9]);
    device.request_shutdown().await;
    write_u32(&handle, 0x220, 9).await?;
    assert_eq!(read_u32(&handle, RESULT).await?, 2);
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_stale_and_invalid_source_commands_preserve_response_and_state() -> TestResult {
    // Given: one successful observation retained as the response baseline.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let handle = device.handle();
    let queue = create_queue(&handle, 1).await?;
    select_completion(&handle, queue).await?;
    write_u32(&handle, 0x220, 7).await?;
    let baseline = response(&handle).await?;

    // When: unknown, stale, and invalid-source commands are issued.
    write_u32(&handle, 0x220, 99).await?;
    assert_eq!(read_u32(&handle, RESULT).await?, 0x11b);
    assert_eq!(read_u64(&handle, 0x228).await?, 99);
    assert_eq!(response(&handle).await?, baseline);
    write_u64(&handle, 0x200, 1).await?;
    write_u32(&handle, 0x220, 2).await?;
    assert_eq!(read_u32(&handle, RESULT).await?, 0x102);
    assert_eq!(
        (
            read_u64(&handle, 0x228).await?,
            read_u64(&handle, 0x230).await?
        ),
        (0, 1)
    );
    assert_eq!(response(&handle).await?, baseline);
    write_u64(&handle, 0x200, 0).await?;
    write_u64(&handle, 0x208, 99).await?;
    write_u32(&handle, 0x220, 2).await?;

    // Then: invalid identity is exact and the real source remains disabled and unchanged.
    assert_eq!(read_u32(&handle, RESULT).await?, 0x104);
    assert_eq!(
        (
            read_u64(&handle, 0x228).await?,
            read_u64(&handle, 0x230).await?
        ),
        (0, 99)
    );
    assert_eq!(response(&handle).await?, baseline);
    assert_eq!(
        handle
            .observe_irq_source(queue.completion_irq_source())
            .await?,
        IrqSourceStatus::try_from(5).map_err(|_| "assigned state")?
    );
    select_completion(&handle, queue).await?;
    write_u32(&handle, 0x220, 2).await?;
    let enabled = response(&handle).await?;
    write_u32(&handle, 0x220, 1).await?;
    assert_eq!(read_u32(&handle, RESULT).await?, 0x105);
    assert_eq!(response(&handle).await?, enabled);
    let malformed = handle.write_mmio(0x210, &2_u32.to_le_bytes()).await;
    assert!(malformed.is_err());
    assert_eq!(read_u32(&handle, RESULT).await?, 0x105);
    assert_eq!(response(&handle).await?, enabled);
    Ok(())
}
