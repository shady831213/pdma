use super::*;
use hardware_interface_runtime::{NoopRegisterObserver, SharedMemory};
use pdma_model_contract::semantic::ResourceConfig;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

#[tokio::test]
async fn pending_lock_acquisition_is_cancel_safe_before_commit() {
    // Given: a real core lock held before an exact register future reaches its commit point.
    let endpoint = MmioRegisterEndpoint::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    ));
    let held = endpoint.shared.state.lock().await;
    {
        let mut response = [0_u8; 4];
        let mut operation = pin!(endpoint.read(0, &mut response));
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        assert!(matches!(
            operation.as_mut().poll(&mut context),
            Poll::Pending
        ));
    }
    assert_eq!(held.register_gate.next_sequence(), Ok(0));
    drop(held);
}

#[tokio::test]
async fn prepared_read_and_write_are_side_effect_free_until_commit() {
    // Given: an unchanged core and generated read/write register identities.
    let endpoint = MmioRegisterEndpoint::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    ));
    let mut state = endpoint.shared.state.lock().await;
    let before = dispatch::read(&state, RegisterId::QId).encode();
    let read = prepare_read(&state, RegisterId::QId, 0x108);
    let write = prepare_write(RegisterId::QId, 7, 0x108, &7_u64.to_le_bytes());
    assert_eq!(read.response, before);
    assert_eq!(dispatch::read(&state, RegisterId::QId).encode(), before);
    assert_eq!(state.register_gate.next_sequence(), Ok(0));
    write.commit(&mut state);
    assert_eq!(
        dispatch::read(&state, RegisterId::QId).encode(),
        7_u64.to_le_bytes()
    );
    drop(state);
}

#[tokio::test]
async fn prepared_batch_write_is_typed_consumed_and_has_no_arbitrary_install_hook() {
    // Given: a fresh core and the two closed queue batch command variants.
    let endpoint = MmioRegisterEndpoint::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
        Arc::new(NoopRegisterObserver),
    ));
    let mut state = endpoint.shared.state.lock().await;
    let bootstrap = BootstrapBatchStaging {
        expected_epoch: 1,
        id: 2,
        cfg_software_owner: 3,
        sq_base: 4,
        sq_depth: 5,
        cq_base: 6,
        cq_depth: 7,
        eq_base: 8,
        eq_depth: 9,
    };
    let submission = SubmissionBatchStaging {
        expected_epoch: 10,
        id: 11,
        expected_cursor: 12,
        new_cursor: 13,
    };
    let activate = prepare_bootstrap_batch_write(bootstrap, QueueResponseBank::default())
        .expect("fixed activate write decodes")
        .commit(&mut state);
    let submission = prepare_submission_batch_write(submission, QueueResponseBank::default())
        .expect("fixed submission write decodes")
        .commit(&mut state);
    assert_eq!(activate.request, 1_u32.to_le_bytes());
    assert_eq!(submission.request, 3_u32.to_le_bytes());
    assert_register(&state, RegisterId::QExpectedEpoch, 10);
    assert_register(&state, RegisterId::QId, 11);
    assert_register(&state, RegisterId::QExpectedCursor, 12);
    assert_register(&state, RegisterId::QNewCursor, 13);
    drop(state);
}

fn assert_register(state: &DeviceCore, register: RegisterId, expected: u64) {
    assert_eq!(
        dispatch::read(state, register).encode(),
        expected.to_le_bytes()
    );
}
