use super::{QueueBootstrapBatch, QueueResponseBytes};
use crate::runtime::AsyncDevice;
use pdma_model_contract::abi::mmio::MmioByteCount;
use pdma_model_contract::semantic::{
    DeviceEpoch, EventRouting, QueueActivation, QueueConfig, QueueId, QueueRingObjects,
    SoftwareOwnerId,
};
use std::future::{Future, poll_fn};
use std::sync::Arc;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

mod cancellation;
mod ordering;
use ordering::CommitOrderObserver;
mod fault_injection;

#[tokio::test]
async fn pending_real_lock_drop_repeats_without_mutation_then_recovers() -> TestResult {
    // Given: committed sentinel bytes and every standalone queue latch are pinned.
    let (device, _worker) = AsyncDevice::from_core(crate::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let handle = device.handle();
    let sentinel = handle.allocate_memory_region(64).await?;
    handle.write_memory(sentinel.base(), &[0x5a; 64]).await?;
    let before = handle.snapshot().await;
    let latches_before = queue_latches(&handle).await?;

    // When: real RunnerState lock contention makes two batch futures pending before commit.
    for _ in 0..2 {
        let proposal = handle.stage_memory_regions(&[128, 64, 128]).await?;
        let activation = activation(&proposal)?;
        let [submission, _, _] = proposal.regions() else {
            return Err("three staged regions required".into());
        };
        let proposed_object = submission.object();
        let state = handle.shared.state.lock().await;
        let wake_before = state.wake_generation;
        let mut batch =
            Box::pin(handle.bootstrap_queue_batch(QueueBootstrapBatch::new(proposal, activation)));
        let pending =
            poll_fn(|context| std::task::Poll::Ready(batch.as_mut().poll(context).is_pending()))
                .await;
        assert!(pending);
        drop(batch);
        assert_eq!(state.dma.generation(), 1);
        assert_eq!(state.dma.region_for_object(proposed_object), None);
        assert_eq!(state.generation, before.generation());
        assert_eq!(state.wake_generation, wake_before);
        assert!(state.exact_ring(activation.queue()).is_err());
        drop(state);
        assert_eq!(
            handle.read_memory(sentinel.base(), 64).await?,
            vec![0x5a; 64]
        );
        assert_eq!(queue_latches(&handle).await?, latches_before);
        assert_eq!(handle.snapshot().await, before);
    }

    // Then: releasing the lock lets a fresh batch commit once with canonical raw bytes.
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = activation(&stage)?;
    let response = handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
        .await?;
    assert_eq!(response, response_latches(&handle).await?);
    assert_eq!(handle.memory_generation().await, 2);
    assert_eq!(
        handle.snapshot().await.generation(),
        before.generation() + 1
    );
    Ok(())
}

fn activation(stage: &crate::PdmaMemoryStage) -> TestResult<QueueActivation> {
    let [submission, completion, event] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    Ok(QueueActivation::new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingObjects::new(submission.object(), completion.object(), event.object()),
    ))
}

async fn queue_latches(handle: &crate::runtime::AsyncDeviceHandle) -> TestResult<Vec<Vec<u8>>> {
    let mut values = Vec::new();
    for (offset, width) in queue_latch_offsets() {
        values.push(handle.read_mmio(offset, width).await?);
    }
    Ok(values)
}

async fn response_latches(
    handle: &crate::runtime::AsyncDeviceHandle,
) -> TestResult<QueueResponseBytes> {
    let mut response = [0; 72];
    for (slot, offset) in response.chunks_exact_mut(8).zip((0x188..=0x1c8).step_by(8)) {
        let value: [u8; 8] = handle
            .read_mmio(offset, MmioByteCount::from(8_u8))
            .await?
            .try_into()
            .map_err(|_| "queue response register was not eight bytes")?;
        slot.copy_from_slice(&value);
    }
    Ok(response.into())
}

fn queue_latch_offsets() -> [(u32, MmioByteCount); 13] {
    [
        (0x16c, MmioByteCount::from(4_u8)),
        (0x170, MmioByteCount::from(8_u8)),
        (0x178, MmioByteCount::from(8_u8)),
        (0x180, MmioByteCount::from(8_u8)),
        (0x188, MmioByteCount::from(8_u8)),
        (0x190, MmioByteCount::from(8_u8)),
        (0x198, MmioByteCount::from(8_u8)),
        (0x1a0, MmioByteCount::from(8_u8)),
        (0x1a8, MmioByteCount::from(8_u8)),
        (0x1b0, MmioByteCount::from(8_u8)),
        (0x1b8, MmioByteCount::from(8_u8)),
        (0x1c0, MmioByteCount::from(8_u8)),
        (0x1c8, MmioByteCount::from(8_u8)),
    ]
}
