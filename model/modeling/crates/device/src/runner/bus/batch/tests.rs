use super::{QueueBootstrapBatch, QueueResponseBytes};
use crate::runner::AsyncDevice;
use pdma_model_contract::abi::mmio::MmioByteCount;
use pdma_model_contract::{DeviceEpoch, QueueActivation, QueueRingRegions, RingLayout, RingRecord};
use pdma_model_contract::{EventRouting, QueueConfig, QueueId, SoftwareOwnerId};
use std::future::{Future, poll_fn};
use std::sync::{Arc, Mutex};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[path = "tests/fault_injection.rs"]
mod fault_injection;

#[tokio::test]
async fn pending_real_lock_drop_repeats_without_mutation_then_recovers() -> TestResult {
    // Given: committed sentinel bytes and every standalone queue latch are pinned.
    let (device, _worker) = AsyncDevice::from_core(crate::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
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
        assert_eq!(state.dma_space.generation(), 1);
        assert_eq!(state.dma_space.region_for_object(proposed_object), None);
        assert_eq!(state.generation, before.generation());
        assert_eq!(state.wake_generation, wake_before);
        assert!(state.bus_control.queue(activation.queue()).is_err());
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

#[derive(Default)]
struct CommitOrderObserver {
    memory: Option<Arc<hardware_interface_runtime::SharedMemory>>,
    wake: Mutex<Option<tokio::sync::watch::Receiver<usize>>>,
    observations: Mutex<Vec<hardware_interface_runtime::RegisterObservation>>,
}

impl hardware_interface_runtime::RegisterObserver for CommitOrderObserver {
    fn observe(&self, observation: hardware_interface_runtime::RegisterObservation) {
        let memory = self.memory.as_ref().expect("observer memory configured");
        assert_eq!(memory.regions().len(), 3);
        let wake = self.wake.lock().expect("wake probe lock available");
        assert!(matches!(
            wake.as_ref().expect("wake probe configured").has_changed(),
            Ok(false)
        ));
        drop(wake);
        self.observations
            .lock()
            .expect("observation probe lock available")
            .push(observation);
    }
}

#[tokio::test]
async fn prepared_installs_precede_observation_which_precedes_wake_publication() -> TestResult {
    // Given: the real core observer inspects committed memory and the real wake receiver.
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(CommitOrderObserver {
        memory: Some(Arc::clone(&memory)),
        ..CommitOrderObserver::default()
    });
    let (device, _worker) = AsyncDevice::from_core(crate::DeviceCore::new(
        memory,
        pdma_model_contract::ResourceConfig::default(),
        observer.clone(),
    ));
    let handle = device.handle();
    *observer.wake.lock().expect("wake probe lock available") =
        Some(handle.shared.changed.subscribe());
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = activation(&stage)?;

    // When: the closed bootstrap batch commits once.
    handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
        .await?;

    // Then: the callback saw installed memory before any wake publication, then return saw wake.
    let observations = observer
        .observations
        .lock()
        .expect("observation probe lock available");
    assert_eq!(observations.len(), 1);
    assert_eq!(
        observations
            .first()
            .ok_or("one ordered observation required")?
            .offset,
        0x168
    );
    drop(observations);
    assert!(matches!(
        observer
            .wake
            .lock()
            .expect("wake probe lock available")
            .as_ref()
            .expect("wake probe configured")
            .has_changed(),
        Ok(true)
    ));
    Ok(())
}

fn activation(stage: &crate::PdmaMemoryStage) -> TestResult<QueueActivation> {
    let [submission, completion, event] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    Ok(QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingRegions::new(
            RingLayout::try_new(*submission, 2, RingRecord::Submission)?,
            RingLayout::try_new(*completion, 2, RingRecord::Completion)?,
            RingLayout::try_new(*event, 2, RingRecord::Event)?,
        ),
    )?)
}

async fn queue_latches(handle: &crate::runner::AsyncDeviceHandle) -> TestResult<Vec<Vec<u8>>> {
    let mut values = Vec::new();
    for (offset, width) in queue_latch_offsets() {
        values.push(handle.read_mmio(offset, width).await?);
    }
    Ok(values)
}

async fn response_latches(
    handle: &crate::runner::AsyncDeviceHandle,
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
    Ok(response)
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
