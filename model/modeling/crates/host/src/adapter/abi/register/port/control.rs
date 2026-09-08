use super::registers::queue_offset;
use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::PortError;
use crate::adapter::abi::RegisterQueueStatus;
use pdma_model_contract::abi::mmio::{
    MmioResult, MmioResultDetails, QueueCommand, QueueRegisters, QueueResponseBank,
    QueueResponseRegisters, ResponseCodecError,
};
use pdma_model_contract::abi::{AbiRingEntry, QueueRingLayouts, RingLayout};
use pdma_model_contract::semantic::{DeviceEpoch, EpochQueue, QueueId, RingCursor, RingCursors};

pub(super) fn response_offsets() -> impl Iterator<Item = usize> {
    QueueResponseRegisters::SPEC
        .placements()
        .iter()
        .map(|placement| queue_offset(QueueRegisters::RESPONSE.offset() + placement.offset()))
}

pub(super) async fn read_status<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    queue: EpochQueue,
    rings: QueueRingLayouts,
) -> PdmaPortResult<RegisterQueueStatus> {
    super::pending::resolve(port).await?;
    stage_queue(port, queue).await?;
    command_status(
        port,
        QueueCommand::ReadStatus,
        rings,
        queue,
        super::pending::Effect::Status,
    )
    .await
}

pub(super) async fn update_cursor<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    update: pdma_model_contract::semantic::QueueCursorUpdate,
    rings: QueueRingLayouts,
    command: QueueCommand,
    current: RingCursor,
    effect: super::pending::Effect,
) -> PdmaPortResult<RegisterQueueStatus> {
    let queue = update.queue();
    super::pending::resolve(port).await?;
    stage_queue(port, queue).await?;
    let before = command_status(
        port,
        QueueCommand::ReadStatus,
        rings,
        queue,
        super::pending::Effect::Status,
    )
    .await?;
    let observed = match command {
        QueueCommand::PublishSqProducer => before.submission().producer(),
        QueueCommand::CaptureSqProducer => before.captured_submission_producer(),
        QueueCommand::PublishCqConsumer => before.completion().consumer(),
        QueueCommand::PublishEqConsumer => before.event().consumer(),
        _ => return Err(PortError::SubmissionState.into()),
    };
    if observed != current {
        return Err(PortError::SubmissionState.into());
    }
    registers::write(
        port,
        queue_offset(QueueRegisters::EXPECTED_CURSOR.offset()),
        current.raw(),
    )
    .await?;
    registers::write(
        port,
        queue_offset(QueueRegisters::NEW_CURSOR.offset()),
        update.cursor().raw(),
    )
    .await?;
    command_status(port, command, rings, queue, effect).await
}

pub(super) async fn stage_queue<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    queue: EpochQueue,
) -> PdmaPortResult<()> {
    registers::write(
        port,
        queue_offset(QueueRegisters::EXPECTED_EPOCH.offset()),
        queue.epoch().raw(),
    )
    .await?;
    let id = u64::try_from(queue.queue_id().raw()).map_err(|_| PortError::SubmissionState)?;
    registers::write(port, queue_offset(QueueRegisters::ID.offset()), id).await
}

pub(super) async fn command_status<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    command: QueueCommand,
    rings: QueueRingLayouts,
    queue: EpochQueue,
    effect: super::pending::Effect,
) -> PdmaPortResult<RegisterQueueStatus> {
    registers::write(
        port,
        queue_offset(QueueRegisters::COMMAND.offset()),
        u64::from(command.code()),
    )
    .await?;
    if let Some(receipt) = effect.receipt() {
        receipt.issued();
    }
    port.with_state(|state| {
        state.pending = Some(super::pending::PendingCommand {
            queue,
            rings,
            effect,
        });
    })?;
    super::pending::resolve(port)
        .await?
        .ok_or_else(|| PortError::SubmissionState.into())
}

pub(super) async fn map_result<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    result: u32,
    queue: EpochQueue,
) -> PdmaPortResult<PortError> {
    let result = MmioResult::try_from(result).map_err(|_| PortError::SubmissionState)?;
    let details = [
        registers::read(port, queue_offset(QueueRegisters::DETAIL0.offset())).await?,
        registers::read(port, queue_offset(QueueRegisters::DETAIL1.offset())).await?,
        registers::read(port, queue_offset(QueueRegisters::DETAIL2.offset())).await?,
    ];
    match (MmioResultDetails { result, details }) {
        MmioResultDetails {
            result: MmioResult::StaleEpoch,
            details: [expected, actual, _],
        } => Ok(PortError::StaleEpoch {
            expected: DeviceEpoch::new(expected),
            actual: DeviceEpoch::new(actual),
        }),
        MmioResultDetails {
            result: MmioResult::InvalidQueue,
            ..
        } => Ok(PortError::UnknownQueue(queue.queue_id())),
        _ => Ok(PortError::SubmissionState),
    }
}

pub(super) fn decode_status(
    bytes: &[u8],
    rings: QueueRingLayouts,
) -> PdmaPortResult<RegisterQueueStatus> {
    let response = QueueResponseBank::from_bytes(bytes).map_err(ResponseCodecError::from)?;
    Ok(RegisterQueueStatus::new(
        EpochQueue::new(
            DeviceEpoch::new(response.epoch.raw()),
            QueueId::new(
                usize::try_from(response.queue_id.raw()).map_err(|_| PortError::SubmissionState)?,
            ),
        ),
        cursors(
            rings.submission(),
            RingCursor::new(response.sq_producer.raw()),
            RingCursor::new(response.sq_consumer.raw()),
        )?,
        RingCursor::new(response.sq_captured.raw()),
        cursors(
            rings.completion(),
            RingCursor::new(response.cq_producer.raw()),
            RingCursor::new(response.cq_consumer.raw()),
        )?,
        cursors(
            rings.event(),
            RingCursor::new(response.eq_producer.raw()),
            RingCursor::new(response.eq_consumer.raw()),
        )?,
    ))
}

fn cursors<R: AbiRingEntry>(
    layout: RingLayout<R>,
    producer: RingCursor,
    consumer: RingCursor,
) -> PdmaPortResult<RingCursors> {
    RingCursors::try_new(layout.depth(), producer, consumer)
        .map_err(|_| PortError::SubmissionState.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::abi::PdmaRegisterPortError;
    use pdma_model_contract::abi::{AbiCommand, AbiCompletion, AbiEvent, DmaAddress, DmaRegion};
    use std::error::Error;

    #[test]
    fn malformed_generated_queue_response_preserves_codec_source() {
        fn layout<R: AbiRingEntry>(base: u64) -> RingLayout<R> {
            let length = RingLayout::<R>::checked_extent(2).expect("test extent must fit");
            let region = DmaRegion::try_new(DmaAddress::new(base), length)
                .expect("test region must be valid");
            RingLayout::try_new(region, 2).expect("test ring must be valid")
        }
        let rings = QueueRingLayouts::new(
            layout::<AbiCommand>(0x1000),
            layout::<AbiCompletion>(0x2000),
            layout::<AbiEvent>(0x3000),
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
