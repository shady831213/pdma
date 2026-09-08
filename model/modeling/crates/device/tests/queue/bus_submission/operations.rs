use super::common::{TestResult, activate, allocate, encode, fixture, run_descriptor};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues,
};
use pdma_model_contract::semantic::{QueueControl, QueueCursorUpdate, ResourceConfig, RingCursor};
use pdma_model_device::{AsyncDevice, PendingDmaResult};

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
        pdma_model_contract::semantic::RingCursor::new(1)
    );
    assert_eq!(result.destinations, [expected_a, expected_b]);
    assert!(!result.traces.is_empty());
    assert_eq!(result.steps, 0);
    assert_eq!(result.native_steps, 1);
    assert!(!result.irq_pending);
    Ok(())
}

pub(in crate::queue) async fn multi_queue_arbitration()
-> TestResult<Vec<pdma_model_contract::semantic::QueueId>> {
    let resources = ResourceConfig::default().with_compute_slots(0);
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        resources,
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
    let vector = encode(AbiCommand::from_payload(
        40,
        AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
            source_a_dma_address: source_a.base().raw(),
            source_b_dma_address: source_b.base().raw(),
            destination_dma_address: vector_destination.base().raw(),
            length: 4.into(),
        }),
    ))?;
    let copy = encode(AbiCommand::from_payload(
        41,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: source_a.base().raw(),
            destination_dma_address: copy_destination.base().raw(),
            length: 4.into(),
        }),
    ))?;
    for (queue, slot, descriptor) in [
        (vector_queue, vector_slot, vector),
        (copy_queue, copy_slot, copy),
    ] {
        handle.write_memory(slot, &descriptor).await?;
        handle
            .control_queue(QueueControl::PublishSubmissionProducer(
                QueueCursorUpdate::new(queue, RingCursor::new(1)),
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
    let snapshot = handle.snapshot().await;
    assert_eq!(vector_status.submission().consumer(), RingCursor::new(0));
    assert_eq!(copy_status.submission().consumer(), RingCursor::new(1));
    assert_eq!(
        (snapshot.steps, snapshot.stalls, snapshot.errors),
        (1, 2, 0)
    );
    assert_eq!(handle.native_steps().await, 1);
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
    assert_eq!(order, [copy_queue.queue_id()]);
    handle.request_shutdown().await;
    worker.await?;
    Ok(order)
}

pub(in crate::queue) async fn zero_row_strided() -> TestResult {
    let fixture = fixture(ResourceConfig::default()).await?;
    let descriptor = encode(AbiCommand::from_payload(
        14,
        AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
            source_dma_address: u64::MAX,
            destination_dma_address: u64::MAX,
            rows: 0.into(),
            row_bytes: u32::MAX.into(),
            source_stride: u32::MAX.into(),
            destination_stride: u32::MAX.into(),
        }),
    ))?;
    let result = run_descriptor(fixture, descriptor, &[]).await?;
    assert!(matches!(
        result.pending,
        Some(PendingDmaResult::Completion(_))
    ));
    assert_eq!(result.consumer, RingCursor::new(1));
    assert_eq!(result.native_steps, 1);
    Ok(())
}

pub(in crate::queue) async fn command(
    case: OperationCase,
    handle: &pdma_model_device::AsyncDeviceHandle,
    source_a: pdma_model_device::MappedDmaRegion,
    source_b: pdma_model_device::MappedDmaRegion,
    destination_a: pdma_model_device::MappedDmaRegion,
    destination_b: pdma_model_device::MappedDmaRegion,
) -> TestResult<(AbiCommand, Vec<u8>, Vec<u8>)> {
    match case {
        OperationCase::Copy => Ok((
            AbiCommand::from_payload(
                10,
                AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                    source_dma_address: source_a.base().checked_add(2)?.raw(),
                    destination_dma_address: destination_a.base().checked_add(1)?.raw(),
                    length: 4.into(),
                }),
            ),
            vec![0, 3, 4, 5, 6, 0, 0, 0],
            vec![0; 8],
        )),
        OperationCase::Strided => Ok((
            AbiCommand::from_payload(
                11,
                AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
                    source_dma_address: source_a.base().checked_add(1)?.raw(),
                    destination_dma_address: destination_a.base().raw(),
                    rows: 2.into(),
                    row_bytes: 2.into(),
                    source_stride: 3.into(),
                    destination_stride: 3.into(),
                }),
            ),
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
                AbiCommand::from_payload(
                    12,
                    AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
                        source_dma_address: source_a.base().checked_add(2)?.raw(),
                        destination_list_dma_address: list.base().raw(),
                        length: 3.into(),
                        destination_count: 2.into(),
                    }),
                ),
                vec![3, 4, 5, 0, 0, 0, 0, 0],
                vec![3, 4, 5, 0, 0, 0, 0, 0],
            ))
        }
        OperationCase::Vector => Ok((
            AbiCommand::from_payload(
                13,
                AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
                    source_a_dma_address: source_a.base().checked_add(1)?.raw(),
                    source_b_dma_address: source_b.base().checked_add(1)?.raw(),
                    destination_dma_address: destination_a.base().checked_add(2)?.raw(),
                    length: 4.into(),
                }),
            ),
            vec![0, 0, 9, 9, 9, 9, 0, 0],
            vec![0; 8],
        )),
    }
}
