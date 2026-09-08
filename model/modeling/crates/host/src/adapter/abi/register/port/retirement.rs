use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::{PortError, PortResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::abi::{
    AbiCompletion, AbiEvent, AbiEventKind, AbiRingEntry, DmaAddress, RingCursor as AbiRingCursor,
    RingLayout,
};
use pdma_model_contract::semantic::{
    BufferId, CommandId, CompletionRecord, CompletionStatus, EventErrorKind, EventRecord,
    QueueConfigHandle, QueueId, RingCursor,
};

pub(super) async fn completion<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<Option<CompletionRecord>> {
    let _window = port.queue_window.lock().await;
    super::pending::recover(port).await?;
    let queue = super::submission::refresh(port, handle).await?;
    if let Some(record) = port.with_state(|state| state.completion.take())? {
        return Ok(Some(record));
    }
    let cursors = queue.status.completion();
    if cursors.producer() == cursors.consumer() {
        return Ok(None);
    }
    let address = slot_address(queue.rings.completion(), cursors.consumer())?;
    let mut bytes = vec![0; usize::from(queue.rings.completion().entry_size())];
    port.validate_owner()?;
    MemoryAccess::read(
        &port.access,
        hardware_interface_runtime::MemoryAddress::new(address.raw()),
        MemoryAlignment::ONE,
        &mut bytes,
    )
    .map_err(crate::adapter::abi::PdmaRegisterPortError::from)?;
    let decoded = AbiCompletion::decode(&bytes).map_err(|_| PortError::SubmissionState)?;
    let record = semantic_completion(decoded)?;
    let next = cursors
        .consumer()
        .checked_increment(1)
        .map_err(|_| PortError::SubmissionState)?;
    commit_consumer(
        port,
        handle,
        cursors.consumer(),
        next,
        pdma_model_contract::abi::mmio::QueueCommand::PublishCqConsumer,
        super::pending::Effect::Completion(record),
    )
    .await?;
    port.with_state(|state| state.completion.take())
}

fn semantic_completion(decoded: AbiCompletion) -> PortResult<CompletionRecord> {
    let queue_id = usize::try_from(decoded.origin_queue_id)
        .map(QueueId::new)
        .map_err(|_| PortError::SubmissionState)?;
    let command_id = usize::try_from(decoded.command_id)
        .map(CommandId::new)
        .map_err(|_| PortError::SubmissionState)?;
    Ok(CompletionRecord::new(
        queue_id,
        command_id,
        CompletionStatus::Success,
    ))
}

pub(super) async fn event<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<Option<EventRecord>> {
    let _window = port.queue_window.lock().await;
    super::pending::recover(port).await?;
    let queue = super::submission::refresh(port, handle).await?;
    if let Some(record) = port.with_state(|state| state.event.take())? {
        return Ok(Some(record));
    }
    let cursors = queue.status.event();
    if cursors.producer() == cursors.consumer() {
        return Ok(None);
    }
    let address = slot_address(queue.rings.event(), cursors.consumer())?;
    let mut bytes = vec![0; usize::from(queue.rings.event().entry_size())];
    port.validate_owner()?;
    MemoryAccess::read(
        &port.access,
        hardware_interface_runtime::MemoryAddress::new(address.raw()),
        MemoryAlignment::ONE,
        &mut bytes,
    )
    .map_err(crate::adapter::abi::PdmaRegisterPortError::from)?;
    let decoded = AbiEvent::decode(&bytes).map_err(|_| PortError::SubmissionState)?;
    let record = semantic_event(port, decoded)?;
    let next = cursors
        .consumer()
        .checked_increment(1)
        .map_err(|_| PortError::SubmissionState)?;
    commit_consumer(
        port,
        handle,
        cursors.consumer(),
        next,
        pdma_model_contract::abi::mmio::QueueCommand::PublishEqConsumer,
        super::pending::Effect::Event(record),
    )
    .await?;
    port.with_state(|state| state.event.take())
}

async fn commit_consumer<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
    expected: RingCursor,
    proposed: RingCursor,
    command: pdma_model_contract::abi::mmio::QueueCommand,
    effect: super::pending::Effect,
) -> PdmaPortResult<()> {
    let queue = port.with_state(|state| state.queue(handle))??;
    let observed = if command == pdma_model_contract::abi::mmio::QueueCommand::PublishCqConsumer {
        queue.status.completion().consumer()
    } else {
        queue.status.event().consumer()
    };
    if observed != expected {
        return Err(PortError::SubmissionState.into());
    }
    let status = super::control::update_cursor(
        port,
        pdma_model_contract::semantic::QueueCursorUpdate::new(queue.status.queue(), proposed),
        queue.rings,
        command,
        expected,
        effect,
    )
    .await?;
    port.with_state(|state| {
        let current = state.queue(handle)?;
        state.queue = Some(super::state::QueueState { status, ..current });
        Ok(())
    })?
}

fn slot_address<R: AbiRingEntry>(
    layout: RingLayout<R>,
    cursor: RingCursor,
) -> PortResult<DmaAddress> {
    layout
        .entry_address(AbiRingCursor::new(cursor.raw()))
        .map_err(|_| PortError::SubmissionState)
}

fn semantic_event<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    event: AbiEvent,
) -> PdmaPortResult<EventRecord> {
    let (kind, locator, detail0, detail1, _, _, _) = event.fault().tuple();
    let locator = DmaAddress::new(locator);
    let region = port.with_state(|state| {
        state
            .regions
            .iter()
            .find(|region| {
                let base = region.base().raw();
                let Some(exclusive_end) = base.checked_add(region.length()) else {
                    return false;
                };
                match kind {
                    AbiEventKind::OutOfBounds => {
                        locator.raw() == exclusive_end && detail0 == region.length()
                    }
                    AbiEventKind::UnmappedDmaAddress
                    | AbiEventKind::RangeOverflow
                    | AbiEventKind::InvalidVersion
                    | AbiEventKind::InvalidSize
                    | AbiEventKind::InvalidOpcode
                    | AbiEventKind::InvalidFlags
                    | AbiEventKind::InvalidReserved
                    | AbiEventKind::InvalidIndirectCount
                    | AbiEventKind::InvalidIndirectAlignment
                    | AbiEventKind::ValueOutOfRange => {
                        base <= locator.raw() && locator.raw() < exclusive_end
                    }
                }
            })
            .copied()
    })?;
    let buffer = region.map_or(
        BufferId::new(usize::MAX),
        crate::adapter::abi::PdmaMappedRegion::object,
    );
    let error_kind = match kind {
        AbiEventKind::UnmappedDmaAddress => EventErrorKind::UnknownBuffer,
        AbiEventKind::OutOfBounds => {
            let region = region.ok_or(PortError::SubmissionState)?;
            let end = detail1
                .checked_sub(region.base().raw())
                .ok_or(PortError::SubmissionState)?;
            EventErrorKind::OutOfBounds {
                len: usize::try_from(detail0).map_err(|_| PortError::RangeOverflow)?,
                end: usize::try_from(end).map_err(|_| PortError::RangeOverflow)?,
            }
        }
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
        pdma_model_contract::semantic::QueueId::new(
            usize::try_from(event.origin_queue_id).map_err(|_| PortError::SubmissionState)?,
        ),
        pdma_model_contract::semantic::CommandId::new(
            usize::try_from(event.command_id).map_err(|_| PortError::SubmissionState)?,
        ),
        error_kind,
        buffer,
    ))
}
