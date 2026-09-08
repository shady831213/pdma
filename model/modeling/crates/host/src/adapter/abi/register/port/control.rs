use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::{PortError, adapter::abi::RegisterQueueStatus};
use pdma_model_contract::abi::{
    AbiRingEntry, QueueRingLayouts, RingLayout, registers::QueueRegisters,
};
use pdma_model_contract::semantic::{EpochQueue, RingCursor, RingCursors};

pub(super) async fn read_status<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    queue: EpochQueue,
    rings: QueueRingLayouts,
) -> PdmaPortResult<RegisterQueueStatus> {
    let mut values = Vec::with_capacity(7);
    for local in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::SQ_CAPTURED.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::CQ_CONSUMER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
        QueueRegisters::EQ_CONSUMER.offset(),
    ] {
        values.push(RingCursor::new(
            registers::read(port, registers::queue_offset(queue.queue_id(), local)?).await?,
        ));
    }
    let [
        sq_producer,
        sq_consumer,
        captured,
        cq_producer,
        cq_consumer,
        eq_producer,
        eq_consumer,
    ] = values.as_slice()
    else {
        return Err(PortError::SubmissionState.into());
    };
    Ok(RegisterQueueStatus::new(
        queue,
        cursors(rings.submission(), *sq_producer, *sq_consumer)?,
        *captured,
        cursors(rings.completion(), *cq_producer, *cq_consumer)?,
        cursors(rings.event(), *eq_producer, *eq_consumer)?,
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
