use super::*;
use crate::device::DeviceCore;
use crate::runtime::MmioRegisterEndpoint;
use hardware_interface_runtime::{RegisterAccess, SharedMemory};
use pdma_model_contract::abi::mmio::MmioAccessError;
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
    let held = std::thread::spawn(move || -> Result<(), &'static str> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|_| "holder runtime")?;
        runtime.block_on(shared.backend.hold_for_test(ready_sender, release_receiver));
        Ok(())
    });
    assert!(ready_receiver.recv().is_ok());
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
    assert!(release_sender.send(()).is_ok());
    assert!(matches!(held.join(), Ok(Ok(()))));
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
            let before = state.core.registers.read(RegisterId::QId);
            let read = prepare_read(&state.core, RegisterId::QId, 0x108);
            let write = prepare_write(RegisterId::QId, 7, 0x108, &7_u64.to_le_bytes());
            assert_eq!(state.core.registers.read(RegisterId::QId), before);
            write.commit(&mut state.core);
            let after = state.core.registers.read(RegisterId::QId);
            (before, read.response, after)
        })
        .await;
    assert_eq!(read_response, before);
    assert_eq!(endpoint.shared.register_gate.next_sequence(), Ok(0));
    assert_eq!(after, 7_u64.to_le_bytes());
}

#[tokio::test]
async fn prepared_batch_write_is_typed_consumed_and_has_no_arbitrary_install_hook()
-> Result<(), MmioAccessError> {
    // Given: a fresh core and the two closed queue batch command variants.
    let endpoint = MmioRegisterEndpoint::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let bootstrap = queue::QueueRequest {
        expected_epoch: 1,
        id: 2,
        expected_cursor: 0,
        new_cursor: 0,
        cfg_software_owner: 3,
        sq_base: 4,
        sq_depth: 5,
        cfg_sq_depth: 5,
        cq_base: 6,
        cq_depth: 7,
        cfg_cq_depth: 7,
        eq_base: 8,
        eq_depth: 9,
        cfg_event_policy: 0,
        command: 0,
    };
    let submission = queue::QueueRequest {
        expected_epoch: 10,
        id: 11,
        expected_cursor: 12,
        new_cursor: 13,
        ..bootstrap
    };
    let (activate, submission, registers) = endpoint
        .shared
        .backend
        .interact(|state| {
            let activate = prepare_bootstrap_batch_write(bootstrap, QueueResponseBank::default())?;
            let activate = activate.commit(&state.core);
            let submission =
                prepare_submission_batch_write(submission, QueueResponseBank::default())?;
            let submission = submission.commit(&state.core);
            let registers = [
                (RegisterId::QExpectedEpoch, 10_u64),
                (RegisterId::QId, 11_u64),
                (RegisterId::QExpectedCursor, 12_u64),
                (RegisterId::QNewCursor, 13_u64),
            ]
            .map(|(register, expected)| (state.core.registers.read(register), expected));
            Ok((activate, submission, registers))
        })
        .await?;
    assert_eq!(activate.request, 1_u32.to_le_bytes());
    assert_eq!(submission.request, 3_u32.to_le_bytes());
    for (actual, expected) in registers {
        assert_eq!(actual, expected.to_le_bytes());
    }
    Ok(())
}

#[test]
fn queue_completion_empties_response_with_result_details_in_one_root_mutation() {
    // Given: a queue response bank with every lane distinct from the empty value.
    let state = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    let response = QueueResponseBank {
        epoch: 1.into(),
        queue_id: 2.into(),
        sq_producer: 3.into(),
        sq_captured: 4.into(),
        sq_consumer: 5.into(),
        cq_producer: 6.into(),
        cq_consumer: 7.into(),
        eq_producer: 8.into(),
        eq_consumer: 9.into(),
    };

    state.registers.view_mut(|mut root| {
        queue::complete(
            &mut root,
            pdma_model_contract::abi::mmio::MmioResultDetails {
                result: pdma_model_contract::abi::mmio::MmioResult::InvalidTransaction,
                details: [10, 11, 12],
            },
            pdma_model_contract::abi::mmio::ResponseUpdate::Replace(response),
        );
    });

    // When: one typed root mutation explicitly empties the prior response.
    state.registers.view_mut(|mut root| {
        queue::complete(
            &mut root,
            pdma_model_contract::abi::mmio::MmioResultDetails {
                result: pdma_model_contract::abi::mmio::MmioResult::Empty,
                details: [13, 14, 15],
            },
            pdma_model_contract::abi::mmio::ResponseUpdate::Empty,
        );
    });

    // Then: the result/detail tuple and all nine response lanes change atomically as typed data.
    assert_eq!(
        [
            RegisterId::QResult,
            RegisterId::QDetail0,
            RegisterId::QDetail1,
            RegisterId::QDetail2,
            RegisterId::QRspEpoch,
            RegisterId::QRspId,
            RegisterId::QRspSqProducer,
            RegisterId::QRspSqCaptured,
            RegisterId::QRspSqConsumer,
            RegisterId::QRspCqProducer,
            RegisterId::QRspCqConsumer,
            RegisterId::QRspEqProducer,
            RegisterId::QRspEqConsumer,
        ]
        .map(|register| state.registers.read(register)),
        [
            2_u32.to_le_bytes().to_vec(),
            13_u64.to_le_bytes().to_vec(),
            14_u64.to_le_bytes().to_vec(),
            15_u64.to_le_bytes().to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
            [0_u8; 8].to_vec(),
        ]
    );
}
