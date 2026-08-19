use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::ModelError;
use crate::register::BusQueueStatus;
use pdma_model_contract::abi::RingCursor;
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueCommand, QueueResponseRegistersView, RegisterId,
    ResponseCodecError,
};
use pdma_model_contract::bridge::{QueueRingRegions, RingCursors};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue, QueueId};

const QUEUE_RESPONSE: [RegisterId; 9] = [
    RegisterId::QRspEpoch,
    RegisterId::QRspId,
    RegisterId::QRspSqProducer,
    RegisterId::QRspSqCaptured,
    RegisterId::QRspSqConsumer,
    RegisterId::QRspCqProducer,
    RegisterId::QRspCqConsumer,
    RegisterId::QRspEqProducer,
    RegisterId::QRspEqConsumer,
];

pub(super) async fn read_status<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    queue: EpochQueue,
    rings: QueueRingRegions,
) -> PdmaPortResult<BusQueueStatus> {
    let _window = port.queue_window.lock().await;
    stage_queue(port, queue).await?;
    command_status(port, QueueCommand::ReadStatus, rings, queue).await
}

pub(super) async fn update_cursor<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    queue: EpochQueue,
    rings: QueueRingRegions,
    command: QueueCommand,
    current: RingCursor,
    proposed: RingCursor,
) -> PdmaPortResult<BusQueueStatus> {
    let _window = port.queue_window.lock().await;
    stage_queue(port, queue).await?;
    let before = command_status(port, QueueCommand::ReadStatus, rings, queue).await?;
    let observed = match command {
        QueueCommand::CaptureSqProducer => before.captured_submission_producer(),
        QueueCommand::PublishCqConsumer => before.completion().consumer(),
        QueueCommand::PublishEqConsumer => before.event().consumer(),
        _ => return Err(ModelError::DmaSubmissionState.into()),
    };
    if observed != current {
        return Err(ModelError::DmaSubmissionState.into());
    }
    registers::write(port, RegisterId::QExpectedCursor, current.raw()).await?;
    registers::write(port, RegisterId::QNewCursor, proposed.raw()).await?;
    command_status(port, command, rings, queue).await
}

async fn stage_queue<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    queue: EpochQueue,
) -> PdmaPortResult<()> {
    registers::write(port, RegisterId::QExpectedEpoch, queue.epoch().raw()).await?;
    let id = u64::try_from(queue.queue_id().raw()).map_err(|_| ModelError::DmaSubmissionState)?;
    registers::write(port, RegisterId::QId, id).await
}

async fn command_status<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    command: QueueCommand,
    rings: QueueRingRegions,
    queue: EpochQueue,
) -> PdmaPortResult<BusQueueStatus> {
    registers::write(port, RegisterId::QCommand, u64::from(command.code())).await?;
    let result = u32::try_from(registers::read(port, RegisterId::QResult).await?)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    if result != MmioResult::Ok.code() {
        return map_result(port, result, queue).await;
    }
    decode_status(&registers::read_bank(port, &QUEUE_RESPONSE).await?, rings)
}

async fn map_result<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    result: u32,
    queue: EpochQueue,
) -> PdmaPortResult<BusQueueStatus> {
    let result = MmioResult::try_from(result).map_err(|_| ModelError::DmaSubmissionState)?;
    let details = [
        registers::read(port, RegisterId::QDetail0).await?,
        registers::read(port, RegisterId::QDetail1).await?,
        registers::read(port, RegisterId::QDetail2).await?,
    ];
    match (MmioResultDetails { result, details }) {
        MmioResultDetails {
            result: MmioResult::StaleEpoch,
            details: [expected, actual, _],
        } => Err(ModelError::StaleDeviceEpoch {
            expected: DeviceEpoch::new(expected),
            actual: DeviceEpoch::new(actual),
        }
        .into()),
        MmioResultDetails {
            result: MmioResult::InvalidQueue,
            ..
        } => Err(ModelError::UnknownQueue(queue.queue_id()).into()),
        _ => Err(ModelError::DmaSubmissionState.into()),
    }
}

pub(super) fn decode_status(
    bytes: &[u8],
    rings: QueueRingRegions,
) -> PdmaPortResult<BusQueueStatus> {
    let view = QueueResponseRegistersView::new(bytes).map_err(ResponseCodecError::from)?;
    let epoch = view
        .epoch()
        .map_err(ResponseCodecError::from)?
        .value()
        .map_err(ResponseCodecError::from)?;
    let queue_id = view
        .queue_id()
        .map_err(ResponseCodecError::from)?
        .value()
        .map_err(ResponseCodecError::from)?;
    let cursor = |value| -> RingCursor { RingCursor::new(value) };
    Ok(BusQueueStatus::new(
        EpochQueue::new(
            DeviceEpoch::new(epoch),
            QueueId::new(usize::try_from(queue_id).map_err(|_| ModelError::DmaSubmissionState)?),
        ),
        cursors(
            rings.submission(),
            cursor(
                view.sq_producer()
                    .map_err(ResponseCodecError::from)?
                    .value()
                    .map_err(ResponseCodecError::from)?,
            ),
            cursor(
                view.sq_consumer()
                    .map_err(ResponseCodecError::from)?
                    .value()
                    .map_err(ResponseCodecError::from)?,
            ),
        )?,
        cursor(
            view.sq_captured()
                .map_err(ResponseCodecError::from)?
                .value()
                .map_err(ResponseCodecError::from)?,
        ),
        cursors(
            rings.completion(),
            cursor(
                view.cq_producer()
                    .map_err(ResponseCodecError::from)?
                    .value()
                    .map_err(ResponseCodecError::from)?,
            ),
            cursor(
                view.cq_consumer()
                    .map_err(ResponseCodecError::from)?
                    .value()
                    .map_err(ResponseCodecError::from)?,
            ),
        )?,
        cursors(
            rings.event(),
            cursor(
                view.eq_producer()
                    .map_err(ResponseCodecError::from)?
                    .value()
                    .map_err(ResponseCodecError::from)?,
            ),
            cursor(
                view.eq_consumer()
                    .map_err(ResponseCodecError::from)?
                    .value()
                    .map_err(ResponseCodecError::from)?,
            ),
        )?,
    ))
}

fn cursors(
    layout: pdma_model_contract::bridge::RingLayout,
    producer: RingCursor,
    consumer: RingCursor,
) -> PdmaPortResult<RingCursors> {
    RingCursors::try_new(layout, producer, consumer)
        .map_err(|_| ModelError::DmaSubmissionState.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register::PdmaRegisterPortError;
    use pdma_model_contract::abi::{DmaAddress, RingRecord};
    use pdma_model_contract::bridge::{PdmaMemoryRegion, RingLayout};
    use pdma_model_contract::semantic::BufferId;
    use std::error::Error;

    #[test]
    fn malformed_generated_queue_response_preserves_codec_source() {
        let layout = |id: usize, base: u64, record: RingRecord| {
            let length = u64::from(record.record_size()) * 2;
            let region =
                PdmaMemoryRegion::try_new(BufferId::new(id), DmaAddress::new(base), length)
                    .expect("test region must be valid");
            RingLayout::try_new(region, 2, record).expect("test ring must be valid")
        };
        let rings = QueueRingRegions::new(
            layout(0, 0x1000, RingRecord::Submission),
            layout(1, 0x2000, RingRecord::Completion),
            layout(2, 0x3000, RingRecord::Event),
        );
        let error = decode_status(&[], rings).expect_err("short generated bank must fail");
        assert!(matches!(error, PdmaRegisterPortError::Response(_)));
        assert!(
            Error::source(&error)
                .and_then(|source| source.downcast_ref::<ResponseCodecError>())
                .is_some()
        );
    }
}
