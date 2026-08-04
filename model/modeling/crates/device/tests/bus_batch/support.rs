use pdma_model_contract::abi::mmio::{MmioByteCount, QueueResponseBank};
use pdma_model_contract::abi::{
    DeviceEpoch, DmaRegion, EpochQueue, QueueActivation, QueueControl, QueueRingRegions,
    RingCursor, RingLayout, RingRecord,
};
use pdma_model_contract::{EventRouting, QueueConfig, QueueId, SoftwareOwnerId};
use pdma_model_device::{
    AsyncDeviceHandle, DmaStage, DmaWrite, QueueBootstrapBatch, SubmissionBatch,
};

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub fn new_handle() -> AsyncDeviceHandle {
    let (device, _worker) = pdma_model_device::AsyncDevice::queue_port();
    device.handle()
}

pub fn activation(
    stage: &DmaStage,
    epoch: DeviceEpoch,
) -> Result<QueueActivation, pdma_model_contract::abi::AbiControlError> {
    let [submission, completion, event] = stage.regions() else {
        return Err(
            pdma_model_contract::abi::AbiControlError::QueueDepthMismatch {
                ring: "staged",
                expected: 3,
                actual: u32::try_from(stage.regions().len()).unwrap_or(u32::MAX),
            },
        );
    };
    QueueActivation::try_new(
        epoch,
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingRegions::new(
            RingLayout::try_new(*submission, 2, RingRecord::Submission)?,
            RingLayout::try_new(*completion, 2, RingRecord::Completion)?,
            RingLayout::try_new(*event, 2, RingRecord::Event)?,
        ),
    )
}

pub async fn bootstrap(
    handle: &AsyncDeviceHandle,
) -> TestResult<(EpochQueue, DmaRegion, [u8; 72])> {
    let stage = handle.stage_dma_regions(&[128, 64, 128]).await?;
    let [submission, _, _] = stage.regions() else {
        return Err("three staged regions required".into());
    };
    let submission = *submission;
    let activation = activation(&stage, DeviceEpoch::initial())?;
    let response = handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
        .await?;
    Ok((activation.queue(), submission, response))
}

pub fn response(status: pdma_model_device::QueueStatus) -> QueueResponseBank {
    QueueResponseBank {
        epoch: status.queue().epoch().raw(),
        queue_id: u64::try_from(status.queue().queue_id().raw()).unwrap_or(u64::MAX),
        sq_producer: status.submission().producer().raw(),
        sq_captured: status.captured_submission_producer().raw(),
        sq_consumer: status.submission().consumer().raw(),
        cq_producer: status.completion().producer().raw(),
        cq_consumer: status.completion().consumer().raw(),
        eq_producer: status.event().producer().raw(),
        eq_consumer: status.event().consumer().raw(),
    }
}

pub async fn queue_latches(handle: &AsyncDeviceHandle) -> TestResult<Vec<Vec<u8>>> {
    let mut values = Vec::new();
    for (offset, width) in [
        (0x16c, MmioByteCount::from(4_u8)),
        (0x170, MmioByteCount::from(8_u8)),
        (0x178, MmioByteCount::from(8_u8)),
        (0x180, MmioByteCount::from(8_u8)),
        (0x188, MmioByteCount::from(8_u8)),
    ] {
        values.push(handle.read_mmio(offset, width).await?);
    }
    Ok(values)
}

pub async fn queue_status(
    handle: &AsyncDeviceHandle,
    queue: EpochQueue,
) -> TestResult<pdma_model_device::QueueStatus> {
    Ok(handle
        .control_queue(QueueControl::ReadStatus(queue))
        .await?)
}

pub fn submission(
    queue: EpochQueue,
    expected: RingCursor,
    address: pdma_model_contract::abi::DmaAddress,
    byte: u8,
) -> SubmissionBatch {
    SubmissionBatch::new(
        queue,
        expected,
        DmaWrite::new(address, vec![byte; 64]),
        None,
    )
}
