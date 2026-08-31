use std::error::Error;
use std::sync::Arc;

use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory};
use pdma_model_contract::abi::{AbiControlError, DmaAddress};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochQueue, QueueControl, QueueCursorUpdate, QueueId, ResourceConfig, RingCursor,
};
use pdma_model_device::test_support::{DeviceStateSnapshot, async_state_snapshot};
use pdma_model_device::{
    AsyncDevice, AsyncDeviceHandle, DmaWrite, PdmaControlError, SubmissionAcceptance,
};

use super::{TestResult, require_error};

#[tokio::test(flavor = "current_thread")]
async fn control_fate_rows_have_literal_sources_and_precedence() -> TestResult {
    // Given: a public async device with a real uncommitted activation and active queue.
    let (device, _worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    ));
    let handle = device.handle();
    let uncommitted = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let uncommitted_activation = super::activation(uncommitted.regions(), DeviceEpoch::initial())?;
    let before = async_state_snapshot(&handle).await?;
    let dma = require_error(handle.activate_mapped_queue(uncommitted_activation).await)?;
    assert_eq!(
        dma,
        PdmaControlError::Dma(pdma_model_device::PdmaMemoryError::InvalidRegion)
    );
    assert_eq!(
        dma.source().and_then(|source| source.downcast_ref()),
        Some(&pdma_model_device::PdmaMemoryError::InvalidRegion)
    );
    assert_state_unchanged(&handle, &before).await?;

    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = super::activation(stage.regions(), DeviceEpoch::initial())?;
    let queue = handle.activate_queue(stage, activation).await?.queue();
    assert_remaining_control_fates(&handle, queue).await
}

async fn assert_remaining_control_fates(
    handle: &pdma_model_device::AsyncDeviceHandle,
    queue: EpochQueue,
) -> TestResult {
    let abi_before = async_state_snapshot(handle).await?;
    let abi = require_error(
        handle
            .control_queue(QueueControl::PublishCompletionProducer(
                QueueCursorUpdate::new(queue, RingCursor::new(3)),
            ))
            .await,
    )?;
    assert_eq!(
        abi,
        PdmaControlError::Abi(AbiControlError::CursorDistanceExceeded {
            producer: 3,
            consumer: 0,
            depth: 2,
        })
    );
    assert_eq!(
        abi.source().and_then(|source| source.downcast_ref()),
        Some(&AbiControlError::CursorDistanceExceeded {
            producer: 3,
            consumer: 0,
            depth: 2,
        })
    );
    assert_state_unchanged(handle, &abi_before).await?;

    let stale_before = async_state_snapshot(handle).await?;
    let stale_control = require_error(
        handle
            .control_queue(QueueControl::ReadStatus(EpochQueue::new(
                DeviceEpoch::new(1),
                QueueId::new(99),
            )))
            .await,
    )?;
    assert_eq!(
        stale_control,
        PdmaControlError::StaleEpoch {
            expected: DeviceEpoch::initial(),
            actual: DeviceEpoch::new(1),
        }
    );
    assert!(stale_control.source().is_none());
    assert_state_unchanged(handle, &stale_before).await?;

    let invalid_queue_before = async_state_snapshot(handle).await?;
    let invalid_queue = require_error(
        handle
            .control_queue(QueueControl::ReadStatus(EpochQueue::new(
                DeviceEpoch::initial(),
                QueueId::new(1),
            )))
            .await,
    )?;
    assert_eq!(invalid_queue, PdmaControlError::InvalidQueue);
    assert!(invalid_queue.source().is_none());
    assert_state_unchanged(handle, &invalid_queue_before).await?;

    let transaction_before = async_state_snapshot(handle).await?;
    let invalid_transaction = require_error(
        handle
            .accept_submission(SubmissionAcceptance::new(
                queue,
                RingCursor::new(1),
                DmaWrite::new(DmaAddress::new(0x1_0000), vec![0; 64]),
                None,
            ))
            .await,
    )?;
    assert_eq!(invalid_transaction, PdmaControlError::InvalidTransaction);
    assert!(invalid_transaction.source().is_none());
    assert_state_unchanged(handle, &transaction_before).await?;
    Ok(())
}

async fn assert_state_unchanged(
    handle: &AsyncDeviceHandle,
    before: &DeviceStateSnapshot,
) -> TestResult {
    let after = async_state_snapshot(handle).await?;
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
    assert_eq!(after.reset_runner_calls, before.reset_runner_calls);
    Ok(())
}
