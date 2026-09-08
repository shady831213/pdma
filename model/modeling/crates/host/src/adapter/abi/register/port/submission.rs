use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::{PortError, PortResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::RingCursor as AbiRingCursor;
use pdma_model_contract::semantic::{Command, QueueConfigHandle};

pub(super) async fn submit<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
    command: Command,
    receipt: Option<crate::semantic::SubmissionReceipt>,
) -> PdmaPortResult<()> {
    let _window = port.queue_window.lock().await;
    let queue = refresh(port, handle).await?;
    let producer = queue.status.submission().producer();
    let next = producer
        .checked_increment(1)
        .map_err(|_| PortError::SubmissionState)?;
    queue
        .status
        .submission()
        .update_producer(next)
        .map_err(|_| PortError::SubmissionState)?;
    let abi = super::descriptor::encode_command(port, command).await?;
    let layout = queue.rings.submission();
    let mut descriptor = vec![0_u8; usize::from(layout.entry_size())];
    abi.encode(&mut descriptor)
        .map_err(|_| PortError::SubmissionState)?;
    let address = layout
        .entry_address(AbiRingCursor::new(producer.raw()))
        .map_err(|_| PortError::SubmissionState)?;
    port.validate_owner()?;
    MemoryAccess::write(
        &port.access,
        MemoryAddress::new(address.raw()),
        MemoryAlignment::ONE,
        &descriptor,
    )?;
    let status = super::control::update_cursor(
        port,
        pdma_model_contract::semantic::QueueCursorUpdate::new(queue.status.queue(), next),
        queue.rings,
        pdma_model_contract::abi::mmio::QueueCommand::PublishSqProducer,
        producer,
        super::pending::Effect::Submit(receipt),
    )
    .await?;
    Ok(port.with_state(|state| -> PortResult<()> {
        let active = state.queue(handle)?;
        state.queue = Some(super::state::QueueState { status, ..active });
        Ok(())
    })??)
}

pub(super) async fn doorbell<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<()> {
    let _window = port.queue_window.lock().await;
    let queue = refresh(port, handle).await?;
    let current = queue.status.captured_submission_producer();
    let proposed = queue.status.submission().producer();
    let status = super::control::update_cursor(
        port,
        pdma_model_contract::semantic::QueueCursorUpdate::new(queue.status.queue(), proposed),
        queue.rings,
        pdma_model_contract::abi::mmio::QueueCommand::CaptureSqProducer,
        current,
        super::pending::Effect::Status,
    )
    .await?;
    Ok(port.with_state(|state| -> PortResult<()> {
        let active = state.queue(handle)?;
        state.queue = Some(super::state::QueueState { status, ..active });
        Ok(())
    })??)
}

pub(super) async fn refresh<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<super::state::QueueState> {
    let queue = port.with_state(|state| state.queue(handle))??;
    let status = super::control::read_status(port, queue.status.queue(), queue.rings).await?;
    port.with_state(|state| {
        let current = state.queue(handle)?;
        let refreshed = super::state::QueueState { status, ..current };
        state.queue = Some(refreshed);
        Ok(refreshed)
    })?
}
