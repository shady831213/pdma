use pdma_model_contract::abi::{
    AbiBroadcastCopy, AbiCommand, AbiId, BroadcastDestinationEntry, BroadcastIovas, BroadcastShape,
    DmaAddress, DmaRegion, EpochQueue, RingRecord,
};
use pdma_model_contract::{
    BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange, BufferId, CommandId,
    ExternalCommand, ModelError, ModelResult,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDevicePort, AsyncDriver, AsyncPdmaBus, BusDevicePort, BusQueueStatus, SoftwareCpu,
};

#[path = "cancellation_support/driver_credit.rs"]
mod driver_credit;

struct SubmissionState {
    status: BusQueueStatus,
    indirect: DmaRegion,
    list_bytes: Vec<u8>,
    descriptor_bytes: Vec<u8>,
}

async fn region(
    handle: &pdma_model_device::AsyncDeviceHandle,
    object: BufferId,
) -> ModelResult<DmaRegion> {
    handle
        .region_for_object(object)
        .await
        .ok_or(ModelError::DmaSubmissionState)
}

async fn dma_bytes(
    handle: &pdma_model_device::AsyncDeviceHandle,
    address: DmaAddress,
    length: u64,
) -> ModelResult<Vec<u8>> {
    handle
        .read_iova(address, length)
        .await
        .map_err(|_| ModelError::DmaSubmissionState)
}

async fn submission_state(
    handle: &pdma_model_device::AsyncDeviceHandle,
    queue: EpochQueue,
    submission: DmaRegion,
) -> ModelResult<SubmissionState> {
    let status = AsyncPdmaBus::read_queue_status(handle, queue)
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let indirect = region(handle, BufferId::new(6)).await?;
    Ok(SubmissionState {
        status,
        list_bytes: dma_bytes(handle, indirect.base(), indirect.length()).await?,
        descriptor_bytes: dma_bytes(
            handle,
            submission.base(),
            u64::from(RingRecord::Submission.record_size()),
        )
        .await?,
        indirect,
    })
}

fn encode_broadcast(
    source: DmaRegion,
    destinations: [DmaRegion; 2],
    indirect: DmaRegion,
) -> ModelResult<(Vec<u8>, Vec<u8>)> {
    let descriptor = AbiCommand::BroadcastCopy(
        AbiBroadcastCopy::try_new(
            AbiId::new(0),
            BroadcastIovas::new(source.base(), indirect.base()),
            BroadcastShape::new(4, 2),
        )
        .map_err(|_| ModelError::DmaSubmissionState)?,
    );
    let mut descriptor_bytes = vec![0; usize::from(RingRecord::Submission.record_size())];
    descriptor
        .encode(&mut descriptor_bytes)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let list_bytes = [
        BroadcastDestinationEntry::new(destinations[0].base())
            .address()
            .raw()
            .to_le_bytes(),
        BroadcastDestinationEntry::new(destinations[1].base())
            .address()
            .raw()
            .to_le_bytes(),
    ]
    .concat();
    Ok((descriptor_bytes, list_bytes))
}

pub(super) async fn verify_cancellation_commit_points() -> ModelResult<()> {
    // Given: a depth-one real ABI bus, an indirect command, and its raw ring mappings.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let cpu = SoftwareCpu::new();
    let (mut port, driver, source, destination_a, destination_b) = cpu
        .run_main(async {
            let mut port = BusDevicePort::new(device.handle())?;
            let driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let source = driver.allocate_buffer_on(&mut port, 4).await?;
            let destination_a = driver.allocate_buffer_on(&mut port, 4).await?;
            let destination_b = driver.allocate_buffer_on(&mut port, 4).await?;
            Ok::<_, ModelError>((port, driver, source, destination_a, destination_b))
        })
        .await?;
    let queue = driver.queue_config_handle();
    let epoch_queue = EpochQueue::new(queue.epoch(), queue.queue_id());
    let submission = region(&handle, BufferId::new(0)).await?;
    let before = AsyncPdmaBus::read_queue_status(&handle, epoch_queue)
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let before_descriptor = dma_bytes(
        &handle,
        submission.base(),
        u64::from(RingRecord::Submission.record_size()),
    )
    .await?;
    let command = ExternalCommand::BroadcastCopy(BroadcastCopyCommand::new(
        CommandId::new(0),
        driver.queue_id(),
        driver.completion_ring_id(),
        BroadcastCopyRequest::new(
            source,
            0,
            4,
            vec![
                BroadcastDestinationRange::new(destination_a, 0),
                BroadcastDestinationRange::new(destination_b, 0),
            ],
        ),
    ));

    // When: acceptance is cancelled before polling, then completed before its separate doorbell.
    cpu.run_main(async {
        drop(port.submit_command(queue, command.clone()));
        Ok::<_, ModelError>(())
    })
    .await?;
    let after_unpolled = AsyncPdmaBus::read_queue_status(&handle, epoch_queue).await;
    assert_eq!(after_unpolled, Ok(before));
    assert_eq!(before.submission().available(), 1);
    assert_eq!(handle.region_for_object(BufferId::new(6)).await, None);
    assert_eq!(
        dma_bytes(
            &handle,
            submission.base(),
            u64::from(RingRecord::Submission.record_size()),
        )
        .await?,
        before_descriptor
    );
    cpu.run_main(port.submit_command(queue, command)).await?;
    cpu.run_main(async {
        drop(port.ring_doorbell(queue));
        drop(port.ring_doorbell(queue));
        Ok::<_, ModelError>(())
    })
    .await?;
    let committed = submission_state(&handle, epoch_queue, submission).await?;
    let source_region = region(&handle, source).await?;
    let destinations = [
        region(&handle, destination_a).await?,
        region(&handle, destination_b).await?,
    ];
    let (expected_descriptor_bytes, expected_list_bytes) =
        encode_broadcast(source_region, destinations, committed.indirect)?;

    // Then: only acceptance commits list mapping/bytes/producer/credit; the dropped doorbell does not.
    assert_eq!(committed.status.submission().producer().raw(), 1);
    assert_eq!(committed.status.submission().available(), 0);
    assert_eq!(committed.status.captured_submission_producer().raw(), 0);
    assert_eq!(committed.list_bytes, expected_list_bytes);
    assert_eq!(committed.descriptor_bytes, expected_descriptor_bytes);
    driver_credit::verify_after_acceptance_driver_credit().await?;
    driver_credit::verify_before_acceptance_driver_credit().await
}
