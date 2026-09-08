#![allow(missing_docs)]

pub(super) mod support;

use pdma_model_contract::abi::AbiCompletion;
use pdma_model_contract::abi::registers::{GlobalRegisters, IrqSourceRegisters, QueueRegisters};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig, irq::IrqNotification};
use pdma_model_device::{MmioRegisterError, PdmaControlError};
use support::*;

#[tokio::test]
async fn source_registers_expose_complete_post_transition_state() -> TestResult {
    // Given: an activated completion source with default routing and mask.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    create_queue(&handle, 1).await?;
    let endpoint = handle.register_endpoint();
    assert_eq!(
        read_u32(&endpoint, i(IrqSourceRegisters::STATUS.offset())?).await?,
        5
    );
    write_u32(&endpoint, i(IrqSourceRegisters::ROUTE_KIND.offset())?, 1).await?;
    write_u64(&endpoint, i(IrqSourceRegisters::ROUTE_VECTOR.offset())?, 33).await?;
    // When: enable, mask, unmask, disable and ack use individual source registers.
    for (offset, value, expected) in [
        (IrqSourceRegisters::ENABLED.offset(), 1, 7),
        (IrqSourceRegisters::MASKED.offset(), 1, 7),
        (IrqSourceRegisters::MASKED.offset(), 0, 3),
        (IrqSourceRegisters::ENABLED.offset(), 0, 1),
        (IrqSourceRegisters::ACK.offset(), 1, 1),
    ] {
        write_u32(&endpoint, i(offset)?, value).await?;
        assert_eq!(
            read_u32(&endpoint, i(IrqSourceRegisters::STATUS.offset())?).await?,
            expected
        );
    }
    // Then: source routing remains intact with no pending messages.
    assert_eq!(
        read_u64(&endpoint, i(IrqSourceRegisters::ROUTE_VECTOR.offset())?).await?,
        33
    );
    assert_eq!(
        read_u64(&endpoint, i(IrqSourceRegisters::PENDING_MESSAGES.offset())?).await?,
        0
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn line_snapshot_claim_and_ack_are_non_consuming_until_records_drain() -> TestResult {
    // Given: one published completion on an enabled unmasked Line source.
    let (device, worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let [submission, completion, _] = create_queue(&handle, 1).await?;
    let endpoint = handle.register_endpoint();
    enable(&endpoint).await?;
    submit_copy(&handle, submission.base(), 0).await?;
    write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 1).await?;
    let worker = tokio::spawn(worker.run());
    device.wait_for_progress_after(0).await;
    // When: repeated snapshots, claims and ACK-before-drain observe the same cause.
    for _ in 0..2 {
        assert!(
            handle
                .irq_snapshot()
                .await
                .ok_or("line snapshot")?
                .asserted()
        );
        assert!(matches!(
            handle
                .take_irq_notification_at_epoch(DeviceEpoch::initial())
                .await?,
            Some(IrqNotification::Line(_))
        ));
        write_u32(&endpoint, i(IrqSourceRegisters::ACK.offset())?, 1).await?;
        assert_eq!(
            read_u32(&endpoint, i(IrqSourceRegisters::STATUS.offset())?).await?,
            123
        );
    }
    // Then: draining the DMA record and acknowledging clears the cause and readiness.
    assert!(
        AbiCompletion::decode(
            &handle
                .read_memory(
                    completion.base(),
                    u64::try_from(AbiCompletion::RECORD_SIZE)?
                )
                .await?
        )
        .is_ok()
    );
    write_u64(&endpoint, q(QueueRegisters::CQ_CONSUMER.offset())?, 1).await?;
    write_u32(&endpoint, i(IrqSourceRegisters::ACK.offset())?, 1).await?;
    assert_eq!(
        read_u32(&endpoint, i(IrqSourceRegisters::STATUS.offset())?).await?,
        3
    );
    assert!(!handle.irq_notification_ready().await);
    assert_eq!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await?,
        None
    );
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn message_readiness_is_non_consuming_and_claim_consumes_exactly_one() -> TestResult {
    // Given: two completions on a Message source masked during publication.
    let (device, worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let [submission, _, _] = create_queue(&handle, 2).await?;
    let endpoint = handle.register_endpoint();
    write_u32(&endpoint, i(IrqSourceRegisters::ROUTE_KIND.offset())?, 1).await?;
    write_u64(&endpoint, i(IrqSourceRegisters::ROUTE_VECTOR.offset())?, 41).await?;
    write_u32(&endpoint, i(IrqSourceRegisters::ENABLED.offset())?, 1).await?;
    for slot in 0..2 {
        submit_copy(&handle, submission.base(), slot).await?;
    }
    write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 2).await?;
    let worker = tokio::spawn(worker.run());
    device.wait_for_progress_after(1).await;
    assert_eq!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await?,
        None
    );
    // When: unmasking exposes messages and repeated readiness checks precede two claims.
    write_u32(&endpoint, i(IrqSourceRegisters::MASKED.offset())?, 0).await?;
    assert!(
        handle
            .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
            .await?
    );
    assert!(handle.irq_notification_ready().await);
    assert!(handle.irq_notification_ready().await);
    for (id, pending, state) in [(0, 1, 91), (1, 0, 75)] {
        assert!(
            handle
                .wait_irq_notification_ready_at_epoch(DeviceEpoch::initial())
                .await?
        );
        let Some(IrqNotification::Message(message)) = handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await?
        else {
            return Err("message notification required".into());
        };
        assert_eq!(message.id().raw(), id);
        assert_eq!(message.vector().raw(), 41);
        assert_eq!(message.source_capability().epoch(), DeviceEpoch::initial());
        assert_eq!(
            read_u64(&endpoint, i(IrqSourceRegisters::PENDING_MESSAGES.offset())?).await?,
            pending
        );
        assert_eq!(
            read_u32(&endpoint, i(IrqSourceRegisters::STATUS.offset())?).await?,
            state
        );
    }
    // Then: the losing claim is empty, including after shutdown.
    assert!(!handle.irq_notification_ready().await);
    assert_eq!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await?,
        None
    );
    device.request_shutdown().await;
    assert_eq!(
        handle
            .take_irq_notification_at_epoch(DeviceEpoch::initial())
            .await?,
        None
    );
    worker.await?;
    Ok(())
}

#[tokio::test]
async fn malformed_stale_and_active_route_writes_preserve_source_state() -> TestResult {
    // Given: an enabled completion source.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    create_queue(&handle, 1).await?;
    let endpoint = handle.register_endpoint();
    enable(&endpoint).await?;
    let before = handle.snapshot().await;
    // When: invalid route values or reconfiguration of an enabled source are attempted.
    assert!(matches!(
        write_u32(&endpoint, i(IrqSourceRegisters::ROUTE_KIND.offset())?, 2).await,
        Err(MmioRegisterError::Access(_))
    ));
    assert!(matches!(
        write_u32(&endpoint, i(IrqSourceRegisters::ROUTE_KIND.offset())?, 1).await,
        Err(MmioRegisterError::Control(
            PdmaControlError::InvalidTransaction
        ))
    ));
    // Then: neither failure changes the source or lifecycle.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        read_u32(&endpoint, i(IrqSourceRegisters::STATUS.offset())?).await?,
        3
    );
    write_u32(
        &endpoint,
        u32::try_from(GlobalRegisters::RESET.offset())?,
        1,
    )
    .await?;
    assert!(matches!(
        write_u32(&endpoint, i(IrqSourceRegisters::ACK.offset())?, 1).await,
        Err(MmioRegisterError::StaleEpoch { .. })
    ));
    Ok(())
}
