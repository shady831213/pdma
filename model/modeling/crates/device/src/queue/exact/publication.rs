use super::{ActiveQueue, RingState};
use crate::queue::io::PublicationOutcome;
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_contract::abi::{AbiCompletion, AbiEvent, AbiRingEntry};

pub(super) fn completion(
    memory: &SharedMemoryAccess,
    active: &mut ActiveQueue,
    record: AbiCompletion,
) -> PublicationOutcome {
    let mut bytes = [0_u8; AbiCompletion::RECORD_SIZE];
    if record.encode(&mut bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    publish(memory, &mut active.completion, &bytes)
}

pub(super) fn event(
    memory: &SharedMemoryAccess,
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
    memory: &SharedMemoryAccess,
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
    if crate::memory::write(memory, address, bytes).is_err() {
        return PublicationOutcome::Retry;
    }
    *ring = updated;
    PublicationOutcome::Published
}
