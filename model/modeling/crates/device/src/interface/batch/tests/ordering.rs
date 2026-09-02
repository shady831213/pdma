use super::*;
use crate::{DmaWrite, SubmissionBatch};
use std::sync::{Arc, Mutex};

pub(super) struct CommitOrderObserver {
    pub(super) memory: Option<Arc<hardware_interface_runtime::SharedMemory>>,
    pub(super) expected_region_count: std::sync::atomic::AtomicUsize,
    pub(super) wake: Mutex<Option<tokio::sync::watch::Receiver<usize>>>,
    pub(super) observations: Mutex<Vec<hardware_interface_runtime::RegisterObservation>>,
}

impl Default for CommitOrderObserver {
    fn default() -> Self {
        Self {
            memory: None,
            expected_region_count: std::sync::atomic::AtomicUsize::new(usize::MAX),
            wake: Mutex::new(None),
            observations: Mutex::new(Vec::new()),
        }
    }
}

impl hardware_interface_runtime::RegisterObserver for CommitOrderObserver {
    fn observe(&self, observation: hardware_interface_runtime::RegisterObservation) {
        let memory = self.memory.as_ref().expect("observer memory configured");
        let expected = self
            .expected_region_count
            .load(std::sync::atomic::Ordering::SeqCst);
        if expected != usize::MAX {
            assert_eq!(memory.regions().len(), expected);
        }
        let wake = self.wake.lock().expect("wake probe lock available");
        if let Some(wake) = wake.as_ref() {
            assert!(matches!(wake.has_changed(), Ok(false)));
        }
        drop(wake);
        self.observations
            .lock()
            .expect("observation probe lock available")
            .push(observation);
    }
}

#[tokio::test]
async fn bootstrap_installs_precede_observation_and_wake() -> TestResult {
    // Given: the observer inspects committed memory and the real wake receiver.
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(CommitOrderObserver {
        memory: Some(Arc::clone(&memory)),
        expected_region_count: std::sync::atomic::AtomicUsize::new(3),
        ..CommitOrderObserver::default()
    });
    let (device, _worker) = AsyncDevice::from_core_observed(
        crate::DeviceCore::new(
            memory,
            pdma_model_contract::semantic::ResourceConfig::default(),
        ),
        observer.clone(),
    );
    let handle = device.handle();
    *observer.wake.lock().expect("wake probe lock available") =
        Some(handle.shared.changed.subscribe());
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = activation(&stage)?;

    // When: the closed bootstrap batch commits once.
    handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
        .await?;

    // Then: the callback saw installed memory before wake publication.
    assert_one_observation_then_wake(&observer, 1_u32.to_le_bytes())?;
    Ok(())
}

#[tokio::test]
async fn submission_installs_precede_observation_and_wake() -> TestResult {
    // Given: an active queue and an observer inspecting its later indirect install.
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(CommitOrderObserver {
        memory: Some(Arc::clone(&memory)),
        expected_region_count: std::sync::atomic::AtomicUsize::new(3),
        ..CommitOrderObserver::default()
    });
    let (device, _worker) = AsyncDevice::from_core_observed(
        crate::DeviceCore::new(
            memory,
            pdma_model_contract::semantic::ResourceConfig::default(),
        ),
        observer.clone(),
    );
    let handle = device.handle();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [descriptor, _, _] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    let descriptor = *descriptor;
    let activation = activation(&stage)?;
    handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
        .await?;
    observer
        .observations
        .lock()
        .expect("observation probe lock available")
        .clear();
    let indirect = handle.stage_memory_regions(&[8]).await?;
    let [indirect_region] = indirect.regions() else {
        return Err("one staged indirect region required".into());
    };
    let indirect_region = *indirect_region;
    observer
        .expected_region_count
        .store(4, std::sync::atomic::Ordering::SeqCst);
    *observer.wake.lock().expect("wake probe lock available") =
        Some(handle.shared.changed.subscribe());

    // When: the submission commits its mapping, bytes, registers, and trace.
    handle
        .submit_queue_batch(SubmissionBatch::new(
            activation.queue(),
            pdma_model_contract::semantic::RingCursor::new(0),
            DmaWrite::new(descriptor.base(), vec![0x6b; 64]),
            Some((
                indirect,
                DmaWrite::new(indirect_region.base(), vec![0xa5; 8]),
            )),
        ))
        .await?;

    // Then: the callback saw all four mappings before wake publication.
    assert_one_observation_then_wake(&observer, 3_u32.to_le_bytes())?;
    Ok(())
}

fn assert_one_observation_then_wake(
    observer: &CommitOrderObserver,
    request: [u8; 4],
) -> TestResult {
    let observations = observer
        .observations
        .lock()
        .expect("observation probe lock available");
    let [observation] = observations.as_slice() else {
        return Err("one batch observation required".into());
    };
    assert_eq!(
        observation.offset,
        crate::interface::mmio::q_command_offset()
    );
    assert_eq!(observation.request_bytes, request);
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
