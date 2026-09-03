use super::*;
use crate::device::DeviceCore;
use crate::runtime::MmioRegisterEndpoint;
use hardware_interface_runtime::{RegisterAccess, SharedMemory};
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
    ));
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let (release_sender, release_receiver) = std::sync::mpsc::channel();
    let shared = Arc::clone(&endpoint.shared);
    let held = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("holder runtime")
            .block_on(shared.backend.hold_for_test(ready_sender, release_receiver));
    });
    ready_receiver.recv().expect("backend holder starts");
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
    assert_eq!(endpoint.shared.register_gate.next_sequence(), Ok(0));
    release_sender.send(()).expect("release holder");
    held.join().expect("holder joins");
}

#[tokio::test]
async fn prepared_read_and_write_are_side_effect_free_until_commit() {
    // Given: an unchanged core and generated read/write register identities.
    let endpoint = MmioRegisterEndpoint::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let (before, read_response, after) = endpoint
        .shared
        .backend
        .interact(|state| {
            let before = dispatch::read(&state.core, RegisterId::QId).encode();
            let read = prepare_read(&state.core, RegisterId::QId, 0x108);
            let write = prepare_write(RegisterId::QId, 7, 0x108, &7_u64.to_le_bytes());
            assert_eq!(
                dispatch::read(&state.core, RegisterId::QId).encode(),
                before
            );
            write.commit(&mut state.core);
            let after = dispatch::read(&state.core, RegisterId::QId).encode();
            (before, read.response, after)
        })
        .await;
    assert_eq!(read_response, before);
    assert_eq!(endpoint.shared.register_gate.next_sequence(), Ok(0));
    assert_eq!(after, 7_u64.to_le_bytes());
}

#[tokio::test]
async fn prepared_batch_write_is_typed_consumed_and_has_no_arbitrary_install_hook() {
    // Given: a fresh core and the two closed queue batch command variants.
    let endpoint = MmioRegisterEndpoint::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
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
    let (activate, submission, registers) = endpoint
        .shared
        .backend
        .interact(|state| {
            let activate = prepare_bootstrap_batch_write(bootstrap, QueueResponseBank::default())
                .expect("fixed activate write decodes")
                .commit(&mut state.core);
            let submission =
                prepare_submission_batch_write(submission, QueueResponseBank::default())
                    .expect("fixed submission write decodes")
                    .commit(&mut state.core);
            let registers = [
                (RegisterId::QExpectedEpoch, 10_u64),
                (RegisterId::QId, 11_u64),
                (RegisterId::QExpectedCursor, 12_u64),
                (RegisterId::QNewCursor, 13_u64),
            ]
            .map(|(register, expected)| (dispatch::read(&state.core, register).encode(), expected));
            (activate, submission, registers)
        })
        .await;
    assert_eq!(activate.request, 1_u32.to_le_bytes());
    assert_eq!(submission.request, 3_u32.to_le_bytes());
    for (actual, expected) in registers {
        assert_eq!(actual, expected.to_le_bytes());
    }
}
