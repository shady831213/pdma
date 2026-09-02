use super::support::{RecordingObserver, TestResult, contend_core, create_message, submit_copy};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, DeviceCore};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

/// Drives malformed and cancellation endpoint paths.
///
/// # Panics
/// Panics when an asserted endpoint invariant is violated.
pub(super) async fn run() -> TestResult {
    // Given: one live endpoint with snapshots and a real pending message available to claim.
    let observer = Arc::new(RecordingObserver::default());
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let (device, worker) = AsyncDevice::from_core_observed(
        DeviceCore::new(memory.clone(), ResourceConfig::default()),
        observer.clone(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();
    let queue = create_message(&handle).await?;
    submit_copy(&handle, queue).await?;
    handle.ring_doorbell(queue).await?;
    device.wait_for_progress_after(0).await;

    // When: malformed operations, precommit cancellation, one claim, and empty revalidation run.
    let before_state = handle.snapshot().await;
    let before_failures = observer.snapshot().len();
    let mut byte = [0_u8; 1];
    let width = endpoint.read(0, &mut byte).await.is_err();
    let mut aligned = [0_u8; 4];
    let alignment = endpoint.read(6, &mut aligned).await.is_err();
    let interior = endpoint.read(12, &mut aligned).await.is_err();
    let permission = endpoint.write(0, &[0; 4]).await.is_err();
    let mask = endpoint
        .write(0x008, &u32::MAX.to_le_bytes())
        .await
        .is_err();
    let failed_observations = observer.snapshot().len() - before_failures;
    let state_unchanged = handle.snapshot().await == before_state;
    let held = contend_core(&handle, memory.clone()).await?;
    let before_cancel = observer.snapshot().len();
    let mut cancelled_response = [0_u8; 4];
    {
        let mut cancelled = Box::pin(endpoint.read(0, &mut cancelled_response));
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        assert!(matches!(
            cancelled.as_mut().poll(&mut context),
            Poll::Pending
        ));
    }
    let cancelled_observations = observer.snapshot().len() - before_cancel;
    held.release()?;
    let held = contend_core(&handle, memory).await?;
    let before_claim_cancel = observer.snapshot();
    let before_claim_sequences = before_claim_cancel
        .iter()
        .map(|item| item.sequence)
        .collect::<Vec<_>>();
    {
        let mut cancelled_claim = Box::pin(endpoint.claim_notification_exact());
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        assert!(matches!(
            cancelled_claim.as_mut().poll(&mut context),
            Poll::Pending
        ));
    }
    let after_claim_cancel = observer.snapshot();
    let after_claim_sequences = after_claim_cancel
        .iter()
        .map(|item| item.sequence)
        .collect::<Vec<_>>();
    assert_eq!(after_claim_cancel.len(), before_claim_cancel.len());
    assert_eq!(after_claim_sequences, before_claim_sequences);
    held.release()?;
    let claimed = endpoint.claim_notification_exact().await?;
    let cancelled_consumed = claimed.is_none();
    let before_empty = observer.snapshot().len();
    let empty = endpoint.claim_notification_exact().await?;
    let empty_items = observer.snapshot().len() - before_empty;
    let epoch_bytes = handle.read_mmio(0x050, 8).await?;
    let epoch = <[u8; 8]>::try_from(epoch_bytes)
        .map(u64::from_le_bytes)
        .is_ok_and(|current| current == queue.epoch().raw());

    // Then: failures and cancellation are silent while empty emits only command and result.
    assert!(
        [width, alignment, interior, permission, mask, epoch]
            .into_iter()
            .all(core::convert::identity)
    );
    assert_eq!(failed_observations, 0);
    assert_eq!(cancelled_observations, 0);
    assert!(!cancelled_consumed);
    assert_eq!(empty_items, 2);
    assert!(empty.is_none());
    assert!(state_unchanged);
    println!(
        "errors=width,alignment,interior,permission,mask,epoch;failed_observations=0;cancelled_observations=0;cancelled_consumed=false;empty_items={};empty_consumed={};empty_result=Empty;state_unchanged=true",
        empty_items,
        empty.is_some(),
    );
    device.request_shutdown().await;
    worker.await?;
    Ok(())
}
