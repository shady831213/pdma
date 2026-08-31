use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::adapter::abi::{PdmaDmaWrite, PdmaSubmissionBatch};
use crate::{PortError, PortResult};
use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::abi::RingCursor as AbiRingCursor;
use pdma_model_contract::semantic::{Command, QueueConfigHandle};

pub(super) async fn submit<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
    command: Command,
) -> PdmaPortResult<()> {
    let queue = refresh(port, handle).await?;
    let (abi, indirect) = super::descriptor::encode_command(port, command).await?;
    let layout = queue.rings.submission();
    let mut descriptor = vec![0_u8; usize::from(layout.entry_size())];
    abi.encode(&mut descriptor)
        .map_err(|_| PortError::SubmissionState)?;
    let producer = queue.status.submission().producer();
    let address = layout
        .entry_address(AbiRingCursor::new(producer.raw()))
        .map_err(|_| PortError::SubmissionState)?;
    let descriptor = PdmaDmaWrite::new(MemoryAddress::new(address.raw()), descriptor);
    let request = PdmaSubmissionBatch::new(queue.status.queue(), producer, descriptor, indirect);
    let bytes = port.poll_batch(port.access.submit_queue(request)).await?;
    let status = super::control::decode_status(bytes.as_bytes(), queue.rings)?;
    Ok(port.with_state(|state| -> PortResult<()> {
        let active = state.queue(handle)?;
        if active.status.submission().producer() != producer {
            return Err(PortError::SubmissionState);
        }
        state.queue = Some(super::state::QueueState { status, ..active });
        Ok(())
    })??)
}

pub(super) async fn doorbell<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
) -> PdmaPortResult<()> {
    let queue = refresh(port, handle).await?;
    let current = queue.status.captured_submission_producer();
    let proposed = queue.status.submission().producer();
    let status = super::control::update_cursor(
        port,
        queue.status.queue(),
        queue.rings,
        pdma_model_contract::abi::mmio::QueueCommand::CaptureSqProducer,
        current,
        proposed,
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
