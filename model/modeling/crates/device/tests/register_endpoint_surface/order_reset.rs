use super::support::{RecordingObserver, TestResult, contend_core};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{DeviceEpoch, ResourceConfig, WholeDeviceReset};
use pdma_model_device::{AsyncDevice, DeviceCore, MmioRegisterEndpoint};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn endpoint_clones_share_order_and_observe_before_return() -> TestResult {
    // Given: two endpoint clones over one core-owned observer and ordered gate.
    let observer = Arc::new(RecordingObserver::default());
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let (device, _worker) = AsyncDevice::from_core(DeviceCore::new(
        memory.clone(),
        ResourceConfig::default(),
        observer.clone(),
    ));
    let handle = device.handle();
    let endpoint = handle.register_endpoint();

    // When: both clones queue behind the same real precommit gate and then commit.
    let held = contend_core(&handle, memory).await?;
    let first = spawn_read(endpoint.clone(), observer.clone(), 0);
    let second = spawn_read(endpoint.clone(), observer.clone(), 4);
    tokio::task::yield_now().await;
    assert!(!first.is_finished());
    assert!(!second.is_finished());
    held.release()?;
    let first_observed_before_return = first.await??;
    let second_observed_before_return = second.await??;

    // Then: both returns trail observation and the shared sequences are unique and ordered.
    let observations = observer.snapshot();
    assert!(first_observed_before_return);
    assert!(second_observed_before_return);
    assert_eq!(observations.len(), 2);
    assert_eq!(
        observations
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    Ok(())
}

fn spawn_read(
    endpoint: MmioRegisterEndpoint,
    observer: Arc<RecordingObserver>,
    offset: u32,
) -> tokio::task::JoinHandle<TestResult<bool>> {
    tokio::spawn(async move {
        let mut response = [0_u8; 4];
        endpoint.read(offset, &mut response).await?;
        Ok(observer.snapshot().iter().any(|item| item.offset == offset))
    })
}

#[tokio::test(flavor = "current_thread")]
async fn endpoint_reset_preserves_history_and_advances_sequence() -> TestResult {
    // Given: one observed endpoint access before whole-device reset.
    let observer = Arc::new(RecordingObserver::default());
    let (device, _worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
        observer.clone(),
    ));
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let mut before = [0_u8; 4];
    endpoint.read(0, &mut before).await?;

    // When: reset advances the epoch and a later access commits through the same endpoint.
    let epoch = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await?;
    let mut after = [0_u8; 4];
    endpoint.read(0, &mut after).await?;

    // Then: reset retained old history and the later sequence is strictly greater and unique.
    let observations = observer.snapshot();
    assert_eq!(epoch, DeviceEpoch::new(1));
    assert_eq!(observations.len(), 2);
    assert_eq!(observations.first().map(|item| item.sequence), Some(0));
    assert_eq!(observations.last().map(|item| item.sequence), Some(1));
    assert_ne!(
        observations.first().map(|item| item.sequence),
        observations.last().map(|item| item.sequence)
    );
    Ok(())
}
