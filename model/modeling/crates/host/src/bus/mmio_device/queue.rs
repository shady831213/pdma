use super::{MmioBusControlError, MmioDeviceBus, MmioDeviceTransport};
use crate::{BusQueueStatus, BusSubmission, QueueControlWrite};
use pdma_model_contract::abi::mmio::{MmioByteCount, MmioResult, MmioResultDetails};
use pdma_model_contract::abi::{
    EpochQueue, QueueActivation, QueueRingRegions, RingCursor, RingCursors,
};

pub(super) async fn bootstrap<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    stage: T::Stage,
    activation: QueueActivation,
) -> Result<BusQueueStatus, MmioBusControlError> {
    let mut window = bus.queue_window.lock().await;
    let rings = activation.rings();
    let bytes = bus.transport.bootstrap_batch(stage, activation).await?;
    let status = decode_status(&bytes, rings)?;
    window.layouts.insert(status.queue(), rings);
    drop(window);
    Ok(status)
}

pub(super) async fn submit<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    request: BusSubmission<T::Stage>,
) -> Result<BusQueueStatus, MmioBusControlError> {
    let window = bus.queue_window.lock().await;
    let queue = request.queue();
    let rings = window
        .layouts
        .get(&queue)
        .copied()
        .ok_or(MmioBusControlError::MalformedResponse)?;
    let bytes = bus.transport.submission_batch(request).await?;
    let status = decode_status(&bytes, rings);
    drop(window);
    status
}

pub(super) async fn read_status<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    queue: EpochQueue,
) -> Result<BusQueueStatus, MmioBusControlError> {
    let window = bus.queue_window.lock().await;
    let rings = window
        .layouts
        .get(&queue)
        .copied()
        .ok_or(MmioBusControlError::MalformedResponse)?;
    stage_queue(bus, queue).await?;
    let status = command_status(bus, 2, rings).await;
    drop(window);
    status
}

pub(super) async fn write_control<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    request: QueueControlWrite,
) -> Result<BusQueueStatus, MmioBusControlError> {
    let window = bus.queue_window.lock().await;
    let rings = window
        .layouts
        .get(&request.queue())
        .copied()
        .ok_or(MmioBusControlError::MalformedResponse)?;
    stage_queue(bus, request.queue()).await?;
    let before = command_status(bus, 2, rings).await?;
    let (command, current, proposed) = command_cursors(request, before);
    write_u64(bus, 0x110, current.raw()).await?;
    write_u64(bus, 0x118, proposed.raw()).await?;
    let status = command_status(bus, command, rings).await;
    drop(window);
    status
}

async fn stage_queue<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    queue: EpochQueue,
) -> Result<(), MmioBusControlError> {
    write_u64(bus, 0x100, queue.epoch().raw()).await?;
    let id = u64::try_from(queue.queue_id().raw())
        .map_err(|_| MmioBusControlError::MalformedResponse)?;
    write_u64(bus, 0x108, id).await
}

async fn command_status<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    command: u32,
    rings: QueueRingRegions,
) -> Result<BusQueueStatus, MmioBusControlError> {
    bus.transport
        .write_mmio(0x168, &command.to_le_bytes())
        .await?;
    let result = read_u32(bus, 0x16c).await?;
    if result != MmioResult::Ok.code() {
        return Err(MmioBusControlError::Result(MmioResultDetails {
            result: MmioResult::try_from(result)
                .map_err(|_| MmioBusControlError::MalformedResponse)?,
            details: [
                read_u64(bus, 0x170).await?,
                read_u64(bus, 0x178).await?,
                read_u64(bus, 0x180).await?,
            ],
        }));
    }
    let mut bytes = [0_u8; 72];
    for (destination, offset) in bytes.chunks_exact_mut(8).zip((0x188..=0x1c8).step_by(8)) {
        destination.copy_from_slice(
            &bus.transport
                .read_mmio(offset, MmioByteCount::from(8_u8))
                .await?,
        );
    }
    decode_status(&bytes, rings)
}

fn decode_status(
    bytes: &[u8],
    rings: QueueRingRegions,
) -> Result<BusQueueStatus, MmioBusControlError> {
    let values = bytes
        .chunks_exact(8)
        .map(|chunk| <[u8; 8]>::try_from(chunk).map(u64::from_le_bytes))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MmioBusControlError::MalformedResponse)?;
    let [epoch, id, sqp, sqc, sqn, cqp, cqn, eqp, eqn] = values.as_slice() else {
        return Err(MmioBusControlError::MalformedResponse);
    };
    let queue = EpochQueue::new(
        pdma_model_contract::abi::DeviceEpoch::new(*epoch),
        pdma_model_contract::QueueId::new(
            usize::try_from(*id).map_err(|_| MmioBusControlError::MalformedResponse)?,
        ),
    );
    let submission = cursors(
        rings.submission(),
        RingCursor::new(*sqp),
        RingCursor::new(*sqn),
    )?;
    let completion = cursors(
        rings.completion(),
        RingCursor::new(*cqp),
        RingCursor::new(*cqn),
    )?;
    let event = cursors(rings.event(), RingCursor::new(*eqp), RingCursor::new(*eqn))?;
    Ok(BusQueueStatus::new(
        queue,
        submission,
        RingCursor::new(*sqc),
        completion,
        event,
    ))
}

fn cursors(
    layout: pdma_model_contract::abi::RingLayout,
    producer: RingCursor,
    consumer: RingCursor,
) -> Result<RingCursors, MmioBusControlError> {
    RingCursors::try_new(layout, producer, consumer)
        .map_err(|_| MmioBusControlError::MalformedResponse)
}

const fn command_cursors(
    request: QueueControlWrite,
    status: BusQueueStatus,
) -> (u32, RingCursor, RingCursor) {
    match request {
        QueueControlWrite::CaptureSubmissionProducer(update) => {
            (4, status.captured_submission_producer(), update.cursor())
        }
        QueueControlWrite::PublishSubmissionConsumer(update) => {
            (5, status.submission().consumer(), update.cursor())
        }
        QueueControlWrite::PublishCompletionProducer(update) => {
            (6, status.completion().producer(), update.cursor())
        }
        QueueControlWrite::PublishCompletionConsumer(update) => {
            (7, status.completion().consumer(), update.cursor())
        }
        QueueControlWrite::PublishEventProducer(update) => {
            (8, status.event().producer(), update.cursor())
        }
        QueueControlWrite::PublishEventConsumer(update) => {
            (9, status.event().consumer(), update.cursor())
        }
    }
}

async fn write_u64<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    offset: u32,
    value: u64,
) -> Result<(), MmioBusControlError> {
    bus.transport
        .write_mmio(offset, &value.to_le_bytes())
        .await?;
    Ok(())
}

async fn read_u32<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    offset: u32,
) -> Result<u32, MmioBusControlError> {
    let bytes = bus
        .transport
        .read_mmio(offset, MmioByteCount::from(4_u8))
        .await?;
    <[u8; 4]>::try_from(bytes.as_slice())
        .map(u32::from_le_bytes)
        .map_err(|_| MmioBusControlError::MalformedResponse)
}

async fn read_u64<T: MmioDeviceTransport>(
    bus: &MmioDeviceBus<T>,
    offset: u32,
) -> Result<u64, MmioBusControlError> {
    let bytes = bus
        .transport
        .read_mmio(offset, MmioByteCount::from(8_u8))
        .await?;
    <[u8; 8]>::try_from(bytes.as_slice())
        .map(u64::from_le_bytes)
        .map_err(|_| MmioBusControlError::MalformedResponse)
}
