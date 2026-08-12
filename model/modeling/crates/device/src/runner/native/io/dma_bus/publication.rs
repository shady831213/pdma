use crate::dma_space::DmaSpace;
use crate::runner::bus::control::{ActiveQueue, BusControlState};
use pdma_model_contract::{AbiEvent, DmaAddress, EpochQueue, encode_completion, encode_event};
use pdma_model_contract::{CompletionRecord, QueueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PublicationOutcome {
    Published,
    Retry,
}

pub(super) fn completion(
    dma_space: &mut DmaSpace,
    control: &mut BusControlState,
    queue_id: QueueId,
    record: CompletionRecord,
) -> PublicationOutcome {
    let queue = EpochQueue::new(control.epoch(), queue_id);
    let Ok(active) = control.queue_mut(queue) else {
        return PublicationOutcome::Retry;
    };
    let Ok(bytes) = encode_completion(record) else {
        return PublicationOutcome::Retry;
    };
    publish(dma_space, active, ResultRing::Completion, &bytes)
}

pub(super) fn event(
    dma_space: &mut DmaSpace,
    control: &mut BusControlState,
    event: AbiEvent,
) -> PublicationOutcome {
    let queue = EpochQueue::new(control.epoch(), event.queue_id());
    let Ok(active) = control.queue_mut(queue) else {
        return PublicationOutcome::Retry;
    };
    let Ok(bytes) = encode_event(event) else {
        return PublicationOutcome::Retry;
    };
    publish(dma_space, active, ResultRing::Event, &bytes)
}

#[derive(Debug, Clone, Copy)]
enum ResultRing {
    Completion,
    Event,
}

fn publish(
    dma_space: &mut DmaSpace,
    active: &mut ActiveQueue,
    ring: ResultRing,
    bytes: &[u8],
) -> PublicationOutcome {
    let cursors = match ring {
        ResultRing::Completion => active.completion,
        ResultRing::Event => active.event,
    };
    if cursors.is_full() {
        return PublicationOutcome::Retry;
    }
    let Ok(next) = cursors.producer().checked_increment(1) else {
        return PublicationOutcome::Retry;
    };
    let Ok(updated) = cursors.update_producer(next) else {
        return PublicationOutcome::Retry;
    };
    let layout = active.activation.rings();
    let layout = match ring {
        ResultRing::Completion => layout.completion(),
        ResultRing::Event => layout.event(),
    };
    let Some(offset) = layout
        .slot(cursors.producer())
        .checked_mul(u64::from(layout.record().record_size()))
    else {
        return PublicationOutcome::Retry;
    };
    let Some(raw) = layout.region().base().raw().checked_add(offset) else {
        return PublicationOutcome::Retry;
    };
    if dma_space.write_iova(DmaAddress::new(raw), bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    match ring {
        ResultRing::Completion => active.completion = updated,
        ResultRing::Event => active.event = updated,
    }
    PublicationOutcome::Published
}
