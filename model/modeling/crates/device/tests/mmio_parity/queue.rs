use super::{TestResult, read_u64, write_u32, write_u64};
use pdma_model_contract::abi::{
    DeviceEpoch, EpochQueue, QueueActivation, QueueControl, QueueCursorUpdate, QueueRingRegions,
    RingCursor, RingLayout, RingRecord,
};
use pdma_model_contract::{EventRouting, QueueConfig, QueueId, SoftwareOwnerId};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};

#[tokio::test(flavor = "current_thread")]
async fn mapped_activation_and_all_cursor_shadows_match_typed_control() -> TestResult {
    // Given: independent typed and raw devices with identical committed ring mappings.
    let (typed_device, _typed_worker) = AsyncDevice::queue_port();
    let (raw_device, _raw_worker) = AsyncDevice::queue_port();
    let typed = typed_device.handle();
    let raw = raw_device.handle();
    let typed_stage = typed.stage_dma_regions(&[128, 64, 128]).await?;
    let raw_stage = raw.stage_dma_regions(&[128, 64, 128]).await?;
    let typed_regions = typed.commit_dma_stage(typed_stage).await?;
    let raw_regions = raw.commit_dma_stage(raw_stage).await?;
    let queue = EpochQueue::new(DeviceEpoch::initial(), QueueId::new(0));

    // When: both surfaces activate and advance every one of the seven cursor shadows.
    let typed_status = typed
        .activate_mapped_queue(activation(&typed_regions)?)
        .await?;
    activate_raw(&raw, &raw_regions).await?;
    assert_eq!(typed_status.queue(), queue);
    assert_eq!(read_response(&raw).await?, [0; 9]);
    for request in cursor_requests(queue) {
        typed.control_queue(request).await?;
    }
    write_u64(&raw, 0x110, 0).await?;
    write_u64(&raw, 0x118, 1).await?;
    for command in 3..=9 {
        write_u32(&raw, 0x168, command).await?;
    }
    let typed_status = typed.control_queue(QueueControl::ReadStatus(queue)).await?;

    // Then: the raw response is the same atomic typed status with literal numeric lanes.
    assert_eq!(
        read_response(&raw).await?,
        [
            typed_status.queue().epoch().raw(),
            u64::try_from(typed_status.queue().queue_id().raw())?,
            typed_status.submission().producer().raw(),
            typed_status.captured_submission_producer().raw(),
            typed_status.submission().consumer().raw(),
            typed_status.completion().producer().raw(),
            typed_status.completion().consumer().raw(),
            typed_status.event().producer().raw(),
            typed_status.event().consumer().raw(),
        ]
    );
    println!("{{\"mapped_activation\":true,\"cursor_shadows\":[1,1,1,1,1,1,1]}}");
    Ok(())
}

async fn activate_raw(
    handle: &AsyncDeviceHandle,
    regions: &[pdma_model_contract::abi::DmaRegion],
) -> TestResult {
    let [submission, completion, event] = regions else {
        return Err("three ring regions required".into());
    };
    for (offset, value) in [
        (0x100, 0),
        (0x108, 0),
        (0x130, 7),
        (0x138, submission.base().raw()),
        (0x148, completion.base().raw()),
        (0x158, event.base().raw()),
    ] {
        write_u64(handle, offset, value).await?;
    }
    for (offset, value) in [(0x120, 2), (0x124, 2), (0x140, 2), (0x150, 2), (0x160, 2)] {
        write_u32(handle, offset, value).await?;
    }
    write_u32(handle, 0x168, 1).await
}

async fn read_response(handle: &AsyncDeviceHandle) -> TestResult<[u64; 9]> {
    let mut response = [0; 9];
    for (slot, offset) in response.iter_mut().zip((0x188..=0x1c8).step_by(8)) {
        *slot = read_u64(handle, offset).await?;
    }
    Ok(response)
}

fn activation(regions: &[pdma_model_contract::abi::DmaRegion]) -> TestResult<QueueActivation> {
    let [submission, completion, event] = regions else {
        return Err("three ring regions required".into());
    };
    Ok(QueueActivation::try_new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
        QueueRingRegions::new(
            RingLayout::try_new(*submission, 2, RingRecord::Submission)?,
            RingLayout::try_new(*completion, 2, RingRecord::Completion)?,
            RingLayout::try_new(*event, 2, RingRecord::Event)?,
        ),
    )?)
}

const fn cursor_requests(queue: EpochQueue) -> [QueueControl; 7] {
    let update = QueueCursorUpdate::new(queue, RingCursor::new(1));
    [
        QueueControl::PublishSubmissionProducer(update),
        QueueControl::CaptureSubmissionProducer(update),
        QueueControl::PublishSubmissionConsumer(update),
        QueueControl::PublishCompletionProducer(update),
        QueueControl::PublishCompletionConsumer(update),
        QueueControl::PublishEventProducer(update),
        QueueControl::PublishEventConsumer(update),
    ]
}
