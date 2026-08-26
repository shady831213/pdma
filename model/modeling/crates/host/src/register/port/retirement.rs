use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::abi::{AbiEventKind, DmaAddress, RingCursor};
use pdma_model_contract::semantic::{
    BufferId, CompletionRecord, EventErrorKind, EventRecord, QueueConfigHandle,
};
use pdma_model_contract::translation::{decode_completion, decode_event};

pub(super) async fn completion<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<Option<CompletionRecord>> {
    let queue = super::submission::refresh(port, handle).await?;
    let cursors = queue.status.completion();
    if cursors.producer() == cursors.consumer() {
        return Ok(None);
    }
    let address = slot_address(queue.rings.completion(), cursors.consumer())?;
    let mut bytes = vec![0; usize::from(queue.rings.completion().record().record_size())];
    port.validate_owner()?;
    MemoryAccess::read(
        &port.access,
        hardware_interface_runtime::MemoryAddress::new(address.raw()),
        MemoryAlignment::ONE,
        &mut bytes,
    )
    .map_err(crate::register::PdmaRegisterPortError::from)?;
    let record = decode_completion(&bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let next = cursors
        .consumer()
        .checked_increment(1)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    commit_consumer(
        port,
        handle,
        cursors.consumer(),
        next,
        pdma_model_contract::abi::mmio::QueueCommand::PublishCqConsumer,
    )
    .await?;
    Ok(Some(record))
}

pub(super) async fn event<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<Option<EventRecord>> {
    let queue = super::submission::refresh(port, handle).await?;
    let cursors = queue.status.event();
    if cursors.producer() == cursors.consumer() {
        return Ok(None);
    }
    let address = slot_address(queue.rings.event(), cursors.consumer())?;
    let mut bytes = vec![0; usize::from(queue.rings.event().record().record_size())];
    port.validate_owner()?;
    MemoryAccess::read(
        &port.access,
        hardware_interface_runtime::MemoryAddress::new(address.raw()),
        MemoryAlignment::ONE,
        &mut bytes,
    )
    .map_err(crate::register::PdmaRegisterPortError::from)?;
    let decoded = decode_event(&bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let record = semantic_event(port, decoded)?;
    let next = cursors
        .consumer()
        .checked_increment(1)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    commit_consumer(
        port,
        handle,
        cursors.consumer(),
        next,
        pdma_model_contract::abi::mmio::QueueCommand::PublishEqConsumer,
    )
    .await?;
    Ok(Some(record))
}

async fn commit_consumer<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
    expected: RingCursor,
    proposed: RingCursor,
    command: pdma_model_contract::abi::mmio::QueueCommand,
) -> PdmaPortResult<()> {
    let queue = port.with_state(|state| state.queue(handle))??;
    let observed = if command == pdma_model_contract::abi::mmio::QueueCommand::PublishCqConsumer {
        queue.status.completion().consumer()
    } else {
        queue.status.event().consumer()
    };
    if observed != expected {
        return Err(ModelError::DmaSubmissionState.into());
    }
    let status = super::control::update_cursor(
        port,
        queue.status.queue(),
        queue.rings,
        command,
        expected,
        proposed,
    )
    .await?;
    port.with_state(|state| {
        let current = state.queue(handle)?;
        state.queue = Some(super::state::QueueState { status, ..current });
        Ok(())
    })?
}

fn slot_address(
    layout: pdma_model_contract::binding::RingLayout,
    cursor: RingCursor,
) -> ModelResult<DmaAddress> {
    let offset = layout
        .slot(cursor)
        .checked_mul(u64::from(layout.record().record_size()))
        .ok_or(ModelError::DmaSubmissionState)?;
    layout
        .region()
        .abi_base()
        .checked_add(offset)
        .map_err(|_| ModelError::DmaSubmissionState)
}

fn semantic_event<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    event: pdma_model_contract::binding::AbiEvent,
) -> PdmaPortResult<EventRecord> {
    let (kind, locator, detail0, detail1, _, _, _) = event.fault().tuple();
    let locator = DmaAddress::new(locator);
    let buffer = port.with_state(|state| {
        state
            .regions
            .iter()
            .find(|region| {
                let base = region.base().raw();
                base <= locator.raw() && locator.raw() < base.saturating_add(region.length())
            })
            .map_or(BufferId::new(usize::MAX), |region| region.object())
    })?;
    let error_kind = match kind {
        AbiEventKind::UnmappedDmaAddress => EventErrorKind::UnknownBuffer,
        AbiEventKind::OutOfBounds => EventErrorKind::OutOfBounds {
            len: usize::try_from(detail0).map_err(|_| ModelError::RangeOverflow)?,
            end: usize::try_from(detail1).map_err(|_| ModelError::RangeOverflow)?,
        },
        AbiEventKind::RangeOverflow
        | AbiEventKind::InvalidVersion
        | AbiEventKind::InvalidSize
        | AbiEventKind::InvalidOpcode
        | AbiEventKind::InvalidFlags
        | AbiEventKind::InvalidReserved
        | AbiEventKind::InvalidIndirectCount
        | AbiEventKind::InvalidIndirectAlignment
        | AbiEventKind::ValueOutOfRange => EventErrorKind::RangeOverflow,
    };
    Ok(EventRecord::new(
        event.queue_id(),
        event.command_id(),
        error_kind,
        buffer,
    ))
}
