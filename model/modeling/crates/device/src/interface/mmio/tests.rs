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
async fn completed_register_writes_survive_later_rejection_and_unpolled_write()
-> Result<(), MmioRegisterError> {
    // Given: a CPU-independent endpoint with separately committed request registers.
    let endpoint = MmioRegisterEndpoint::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    endpoint.write(0x100, &10_u64.to_le_bytes()).await?;
    endpoint.write(0x108, &11_u64.to_le_bytes()).await?;

    // When: a later access is invalid and another request write is never polled.
    let invalid = endpoint.write(0x10c, &12_u64.to_le_bytes()).await;
    drop(endpoint.write(0x108, &99_u64.to_le_bytes()));

    // Then: the completed register values and only their observation slots persist.
    assert!(matches!(invalid, Err(MmioRegisterError::Access(_))));
    assert_eq!(endpoint.shared.register_gate.next_sequence(), Ok(2));
    for (offset, expected) in [(0x100, 10_u64), (0x108, 11_u64)] {
        let mut bytes = [0; 8];
        endpoint.read(offset, &mut bytes).await?;
        assert_eq!(u64::from_le_bytes(bytes), expected);
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
