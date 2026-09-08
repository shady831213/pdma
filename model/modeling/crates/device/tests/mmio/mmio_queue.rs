#![allow(missing_docs)]

mod activation_rejections;
pub(super) mod support;

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{GlobalRegisters, QueueRegisters};
use pdma_model_contract::abi::{AbiControlError, AbiEvent};
use pdma_model_contract::semantic::{DeviceEpoch, ResourceConfig};
use pdma_model_device::{MmioRegisterError, PdmaControlError};
use support::*;

#[tokio::test]
async fn raw_activation_uses_exact_committed_mappings_and_returns_zero_cursors() -> TestResult {
    // Given: exact committed depth-two SQ/CQ/EQ mappings.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    // When: the queue is enabled through its direct configuration registers.
    let _regions = create_queue(&handle, 2).await?;
    // Then: the live cursor registers start at zero.
    let endpoint = handle.register_endpoint();
    assert_eq!(
        read_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?).await?,
        1
    );
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    Ok(())
}

#[tokio::test]
async fn stale_activation_rejects_and_fresh_epoch_reuses_committed_mappings() -> TestResult {
    // Given: an enabled queue and committed mappings retained across reset.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let regions = create_queue(&handle, 2).await?;
    let stale = handle.register_endpoint();
    write_u32(&stale, u32::try_from(GlobalRegisters::RESET.offset())?, 1).await?;
    let fresh = stale.at_epoch(DeviceEpoch::new(1));
    program_activation(&fresh, &regions, 2).await?;
    let before = handle.snapshot().await;
    // When: an old endpoint attempts activation and producer publication.
    for (offset, bytes) in [
        (
            q(QueueRegisters::ENABLE.offset())?,
            1_u32.to_le_bytes().to_vec(),
        ),
        (
            q(QueueRegisters::SQ_PRODUCER.offset())?,
            1_u64.to_le_bytes().to_vec(),
        ),
    ] {
        assert_eq!(
            stale.write(offset, &bytes).await,
            Err(MmioRegisterError::StaleEpoch {
                expected: DeviceEpoch::new(1),
                actual: DeviceEpoch::initial(),
            })
        );
    }
    // Then: stale writes leave the queue inactive, and explicit rebinding can enable it.
    assert_eq!(handle.snapshot().await, before);
    assert_eq!(
        read_u32(&fresh, q(QueueRegisters::ENABLE.offset())?).await?,
        0
    );
    write_u32(&fresh, q(QueueRegisters::ENABLE.offset())?, 1).await?;
    assert_eq!(cursors(&fresh).await?, [0; 7]);
    Ok(())
}

#[tokio::test]
async fn direct_producer_rejects_regression_and_excess_distance_without_mutation() -> TestResult {
    // Given: one active depth-two queue with producer one.
    let (device, _worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let _regions = create_queue(&handle, 2).await?;
    let endpoint = handle.register_endpoint();
    let producer = q(QueueRegisters::SQ_PRODUCER.offset())?;
    write_u64(&endpoint, producer, 1).await?;
    let before = cursors(&endpoint).await?;
    // When: the producer regresses or advances beyond capacity.
    for value in [0, u64::MAX] {
        assert!(matches!(
            write_u64(&endpoint, producer, value).await,
            Err(MmioRegisterError::Control(PdmaControlError::Abi(_)))
        ));
    }
    // Then: only the accepted producer is visible and hardware-owned cursors are read-only.
    assert_eq!(cursors(&endpoint).await?, before);
    for offset in [
        QueueRegisters::SQ_CAPTURED.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
    ] {
        assert!(matches!(
            write_u64(&endpoint, q(offset)?, 1).await,
            Err(MmioRegisterError::Access(_))
        ));
    }
    assert_eq!(cursors(&endpoint).await?, [1, 0, 0, 0, 0, 0, 0]);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn raw_publication_precedes_doorbell_and_malformed_descriptor_retires_as_event() -> TestResult
{
    // Given: an active queue containing a malformed descriptor.
    let (device, worker) = device(ResourceConfig::default());
    let handle = device.handle();
    let [submission, _, event] = create_queue(&handle, 2).await?;
    let endpoint = handle.register_endpoint();
    handle.write_memory(submission.base(), &[0; 64]).await?;
    // When: doorbell-before-publication rejects, then publication and capture succeed.
    assert_eq!(
        write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 1).await,
        Err(MmioRegisterError::Control(
            PdmaControlError::InvalidTransaction
        ))
    );
    assert_eq!(cursors(&endpoint).await?, [0; 7]);
    write_u64(&endpoint, q(QueueRegisters::SQ_PRODUCER.offset())?, 1).await?;
    assert_eq!(cursors(&endpoint).await?, [1, 0, 0, 0, 0, 0, 0]);
    write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 1).await?;
    let worker = tokio::spawn(worker.run());
    let snapshot = handle.wait_for_error_after(0).await;
    // Then: one SQ slot is consumed and a fault event, with no completion, is published.
    assert_eq!((snapshot.steps, snapshot.errors), (0, 1));
    assert_eq!(cursors(&endpoint).await?, [1, 1, 1, 0, 0, 1, 0]);
    assert!(
        AbiEvent::decode(
            &handle
                .read_memory(event.base(), u64::try_from(AbiEvent::RECORD_SIZE)?)
                .await?
        )
        .is_ok()
    );
    write_u64(&endpoint, q(QueueRegisters::EQ_CONSUMER.offset())?, 1).await?;
    assert!(matches!(
        write_u64(&endpoint, q(QueueRegisters::EQ_CONSUMER.offset())?, 0).await,
        Err(MmioRegisterError::Control(PdmaControlError::Abi(
            AbiControlError::CursorUpdateRegression { .. }
        )))
    ));
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
