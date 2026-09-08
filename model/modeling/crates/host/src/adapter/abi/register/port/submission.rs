use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::{Command, PortError, QueueConfigHandle};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{RingCursor as AbiRingCursor, registers::QueueRegisters};

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
        .map_err(|_| PortError::RingFull)?;
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
    let result = registers::write(
        port,
        registers::queue_offset(handle.queue_id(), QueueRegisters::SQ_PRODUCER.offset())?,
        next.raw(),
    )
    .await;
    if registers::committed(&result) {
        if let Some(receipt) = receipt {
            receipt.accept();
        }
    }
    result
}

pub(super) async fn doorbell<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<()> {
    let _window = port.queue_window.lock().await;
    let queue = refresh(port, handle).await?;
    registers::write(
        port,
        registers::queue_offset(handle.queue_id(), QueueRegisters::DOORBELL.offset())?,
        queue.status.submission().producer().raw(),
    )
    .await
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
        state.queues.insert(handle.queue_id(), refreshed);
        Ok(refreshed)
    })?
}
