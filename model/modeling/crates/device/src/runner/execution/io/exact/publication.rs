use super::super::PublicationOutcome;
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::adapter::bus::control::{BusControlState, RingState};
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, AbiRingEntry};
use pdma_model_contract::semantic::EpochQueue;
use pdma_model_contract::semantic::{CompletionRecord, QueueId};

pub(super) fn completion(
    dma_space: &PdmaMemoryMappings,
    control: &mut BusControlState,
    queue_id: QueueId,
    record: CompletionRecord,
) -> PublicationOutcome {
    let queue = EpochQueue::new(control.epoch(), queue_id);
    let Ok(active) = control.queue_mut(queue) else {
        return PublicationOutcome::Retry;
    };
    let Ok(queue_id) = u64::try_from(record.queue_id().raw()) else {
        return PublicationOutcome::Retry;
    };
    let Ok(command_id) = u64::try_from(record.command_id().raw()) else {
        return PublicationOutcome::Retry;
    };
    let exact = AbiCompletion::success(queue_id, command_id);
    let mut bytes = [0_u8; AbiCompletion::RECORD_SIZE];
    if exact.encode(&mut bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    publish(dma_space, &mut active.completion, &bytes)
}

pub(super) fn event(
    dma_space: &PdmaMemoryMappings,
    control: &mut BusControlState,
    event: AbiEvent,
) -> PublicationOutcome {
    let Ok(queue_id) = usize::try_from(event.origin_queue_id).map(QueueId::new) else {
        return PublicationOutcome::Retry;
    };
    let queue = EpochQueue::new(control.epoch(), queue_id);
    let Ok(active) = control.queue_mut(queue) else {
        return PublicationOutcome::Retry;
    };
    let mut bytes = [0_u8; AbiEvent::RECORD_SIZE];
    if event.encode(&mut bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    publish(dma_space, &mut active.event, &bytes)
}

fn publish<R: AbiRingEntry>(
    dma_space: &PdmaMemoryMappings,
    ring: &mut RingState<R>,
    bytes: &[u8],
) -> PublicationOutcome {
    if ring.is_full() || bytes.len() != usize::from(ring.layout().entry_size()) {
        return PublicationOutcome::Retry;
    }
    let Ok(next) = ring.producer().checked_increment(1) else {
        return PublicationOutcome::Retry;
    };
    let Ok(updated) = ring.update_producer(next) else {
        return PublicationOutcome::Retry;
    };
    let Ok(address) = ring.entry_address(ring.producer()) else {
        return PublicationOutcome::Retry;
    };
    if dma_space.write_memory(address, bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    *ring = updated;
    PublicationOutcome::Published
}
