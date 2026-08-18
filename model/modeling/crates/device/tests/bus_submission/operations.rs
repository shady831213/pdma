use super::common::{TestResult, activate, allocate, encode, fixture, run_descriptor};
use pdma_model_contract::abi::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiId, AbiStridedCopy, AbiVectorAdd,
};
use pdma_model_contract::abi::{
    BroadcastIovas, BroadcastShape, CopyIovas, CopyStrides, RingCursor, StridedCopyShape,
    VectorIovas,
};
use pdma_model_contract::bridge::{QueueControl, QueueCursorUpdate};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{AsyncDevice, DmaWrite, PendingDmaResult, SubmissionAcceptance};

/// Operation variants covered by the exact descriptor inventory.
#[derive(Clone, Copy, Debug)]
pub enum OperationCase {
    /// Linear copy.
    Copy,
    /// Strided copy.
    Strided,
    /// Broadcast copy.
    Broadcast,
    /// Vector addition.
    Vector,
}

impl OperationCase {
    /// Complete operation inventory.
    pub const ALL: [Self; 4] = [Self::Copy, Self::Strided, Self::Broadcast, Self::Vector];
}

/// Drives one operation through the exact descriptor frontend.
///
/// # Panics
///
/// Panics when the observed exact result violates the pinned operation contract.
pub async fn execute_operation(case: OperationCase) -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let source_a = allocate(&fixture.handle, 8).await?;
    let source_b = allocate(&fixture.handle, 8).await?;
    let destination_a = allocate(&fixture.handle, 8).await?;
    let destination_b = allocate(&fixture.handle, 8).await?;
    fixture
        .handle
        .write_memory(source_a.base(), &[1, 2, 3, 4, 5, 6, 7, 8])
        .await?;
    fixture
        .handle
        .write_memory(source_b.base(), &[8, 7, 6, 5, 4, 3, 2, 1])
        .await?;
    let (command, expected_a, expected_b) = command(
        case,
        &fixture.handle,
        source_a,
        source_b,
        destination_a,
        destination_b,
    )
    .await?;
    let descriptor = encode(command)?;
    let result = run_descriptor(fixture, descriptor, &[destination_a, destination_b]).await?;
    assert!(matches!(
        result.pending,
        Some(PendingDmaResult::Completion(_))
    ));
    assert_eq!(
        result.consumer,
        pdma_model_contract::abi::RingCursor::new(1)
    );
    assert_eq!(result.destinations, [expected_a, expected_b]);
    assert!(!result.traces.is_empty());
    assert_eq!(result.steps, 0);
    assert_eq!(result.native_steps, 1);
    assert!(!result.irq_pending);
    Ok(())
}

pub(super) async fn multi_queue_arbitration()
-> TestResult<Vec<pdma_model_contract::semantic::QueueId>> {
    let resources = ResourceConfig::default().with_compute_slots(0);
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        resources,
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let handle = runner.handle();
    handle.enable_tracing().await;
    let (vector_queue, vector_slot) =
        activate(&handle, pdma_model_contract::semantic::QueueId::new(0)).await?;
    let (copy_queue, copy_slot) =
        activate(&handle, pdma_model_contract::semantic::QueueId::new(1)).await?;
    let source_a = allocate(&handle, 4).await?;
    let source_b = allocate(&handle, 4).await?;
    let vector_destination = allocate(&handle, 4).await?;
    let copy_destination = allocate(&handle, 4).await?;
    handle.write_memory(source_a.base(), &[1, 2, 3, 4]).await?;
    handle.write_memory(source_b.base(), &[4, 3, 2, 1]).await?;
    let vector = encode(AbiCommand::VectorAdd(AbiVectorAdd::new(
        AbiId::new(40),
        VectorIovas::new(source_a.base(), source_b.base(), vector_destination.base()),
        4,
    )))?;
    let copy = encode(AbiCommand::Copy(AbiCopy::new(
        AbiId::new(41),
        CopyIovas::new(source_a.base(), copy_destination.base()),
        4,
    )))?;
    for (queue, slot, descriptor) in [
        (vector_queue, vector_slot, vector),
        (copy_queue, copy_slot, copy),
    ] {
        handle
            .accept_submission(SubmissionAcceptance::new(
                queue,
                RingCursor::new(0),
                DmaWrite::new(slot, descriptor.to_vec()),
                None,
            ))
            .await?;
        handle
            .control_queue(QueueControl::CaptureSubmissionProducer(
                QueueCursorUpdate::new(queue, RingCursor::new(1)),
            ))
            .await?;
    }
    let worker = tokio::spawn(worker.run());
    let _first_stall = handle.wait_for_stall_after(0).await;
    handle.write_memory(source_a.base(), &[1, 2, 3, 4]).await?;
    let _second_stall = handle.wait_for_stall_after(1).await;
    let vector_status = handle
        .control_queue(QueueControl::ReadStatus(vector_queue))
        .await?;
    let copy_status = handle
        .control_queue(QueueControl::ReadStatus(copy_queue))
        .await?;
    assert_eq!(vector_status.submission().consumer(), RingCursor::new(0));
    assert_eq!(copy_status.submission().consumer(), RingCursor::new(1));
    assert_eq!(
        handle
            .read_memory(copy_destination.base(), copy_destination.length())
            .await?,
        [1, 2, 3, 4]
    );
    assert_eq!(
        handle
            .read_memory(vector_destination.base(), vector_destination.length())
            .await?,
        [0; 4]
    );
    let order = handle
        .trace_events()
        .await
        .into_iter()
        .filter(|event| event.kind() == pdma_model_contract::semantic::TraceEventKind::Scheduled)
        .filter_map(|event| event.queue_id())
        .collect();
    handle.request_shutdown().await;
    worker.await?;
    Ok(order)
}

pub(super) async fn command(
    case: OperationCase,
    handle: &pdma_model_device::AsyncDeviceHandle,
    source_a: pdma_model_contract::bridge::PdmaMemoryRegion,
    source_b: pdma_model_contract::bridge::PdmaMemoryRegion,
    destination_a: pdma_model_contract::bridge::PdmaMemoryRegion,
    destination_b: pdma_model_contract::bridge::PdmaMemoryRegion,
) -> TestResult<(AbiCommand, Vec<u8>, Vec<u8>)> {
    match case {
        OperationCase::Copy => Ok((
            AbiCommand::Copy(AbiCopy::new(
                AbiId::new(10),
                CopyIovas::new(
                    source_a.base().checked_add(2)?,
                    destination_a.base().checked_add(1)?,
                ),
                4,
            )),
            vec![0, 3, 4, 5, 6, 0, 0, 0],
            vec![0; 8],
        )),
        OperationCase::Strided => Ok((
            AbiCommand::StridedCopy(AbiStridedCopy::new(
                AbiId::new(11),
                CopyIovas::new(source_a.base().checked_add(1)?, destination_a.base()),
                StridedCopyShape::new(2, 2, CopyStrides::new(3, 3)),
            )),
            vec![2, 3, 0, 5, 6, 0, 0, 0],
            vec![0; 8],
        )),
        OperationCase::Broadcast => {
            let list = allocate(handle, 16).await?;
            let mut entries = [0_u8; 16];
            entries[0..8].copy_from_slice(&destination_a.base().raw().to_le_bytes());
            entries[8..16].copy_from_slice(&destination_b.base().raw().to_le_bytes());
            handle.write_memory(list.base(), &entries).await?;
            Ok((
                AbiCommand::BroadcastCopy(AbiBroadcastCopy::try_new(
                    AbiId::new(12),
                    BroadcastIovas::new(source_a.base().checked_add(2)?, list.base()),
                    BroadcastShape::new(3, 2),
                )?),
                vec![3, 4, 5, 0, 0, 0, 0, 0],
                vec![3, 4, 5, 0, 0, 0, 0, 0],
            ))
        }
        OperationCase::Vector => Ok((
            AbiCommand::VectorAdd(AbiVectorAdd::new(
                AbiId::new(13),
                VectorIovas::new(
                    source_a.base().checked_add(1)?,
                    source_b.base().checked_add(1)?,
                    destination_a.base().checked_add(2)?,
                ),
                4,
            )),
            vec![0, 0, 9, 9, 9, 9, 0, 0],
            vec![0; 8],
        )),
    }
}
