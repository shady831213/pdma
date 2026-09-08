use super::{ActiveQueue, RingState};
use crate::memory::PdmaMemoryMappings;
use crate::queue::io::PublicationOutcome;
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, AbiRingEntry};
use pdma_model_contract::semantic::CompletionRecord;

pub(super) fn completion(
    memory: &PdmaMemoryMappings,
    active: &mut ActiveQueue,
    record: CompletionRecord,
) -> PublicationOutcome {
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
    publish(memory, &mut active.completion, &bytes)
}

pub(super) fn event(
    memory: &PdmaMemoryMappings,
    active: &mut ActiveQueue,
    event: AbiEvent,
) -> PublicationOutcome {
    let mut bytes = [0_u8; AbiEvent::RECORD_SIZE];
    if event.encode(&mut bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    publish(memory, &mut active.event, &bytes)
}

fn publish<R: AbiRingEntry>(
    memory: &PdmaMemoryMappings,
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
    if memory.write_memory(address, bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    *ring = updated;
    PublicationOutcome::Published
}
