use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::register::{PdmaDmaWrite, PdmaPreparedProposal, PdmaRegionRequest, PdmaSubmissionBatch};
use crate::{PortError, PortResult};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues,
    BroadcastDestinationEntry, RingCursor as AbiRingCursor, U32WordValues,
};
use pdma_model_contract::semantic::{
    BroadcastCopyRequest, BufferId, Command, CommandId, CommandOperation, QueueConfigHandle,
};

pub(super) async fn submit<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
    command: Command,
) -> PdmaPortResult<()> {
    let queue = refresh(port, handle).await?;
    let (abi, indirect) = encode_command(port, command).await?;
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

async fn encode_command<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    command: Command,
) -> PdmaPortResult<(AbiCommand, Option<(A::Proposal, PdmaDmaWrite)>)> {
    let command_id = command.command_id();
    match command.into_operation() {
        CommandOperation::Copy(request) => Ok((
            AbiCommand::from_payload(
                abi_id(command_id.raw())?,
                AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                    source_dma_address: operand(port, request.src(), request.src_start().offset())?
                        .raw(),
                    destination_dma_address: operand(
                        port,
                        request.dst(),
                        request.dst_start().offset(),
                    )?
                    .raw(),
                    length: scalar(request.len())?,
                }),
            ),
            None,
        )),
        CommandOperation::StridedCopy(request) => Ok((
            AbiCommand::from_payload(
                abi_id(command_id.raw())?,
                AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
                    source_dma_address: operand(port, request.src(), request.src_start().offset())?
                        .raw(),
                    destination_dma_address: operand(
                        port,
                        request.dst(),
                        request.dst_start().offset(),
                    )?
                    .raw(),
                    rows: scalar(request.rows())?,
                    row_bytes: scalar(request.row_bytes())?,
                    source_stride: scalar(request.src_stride())?,
                    destination_stride: scalar(request.dst_stride())?,
                }),
            ),
            None,
        )),
        CommandOperation::BroadcastCopy(request) => broadcast(port, command_id, &request).await,
        CommandOperation::VectorAdd(request) => Ok((
            AbiCommand::from_payload(
                abi_id(command_id.raw())?,
                AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
                    source_a_dma_address: operand(port, request.src_a(), request.src_a_offset())?
                        .raw(),
                    source_b_dma_address: operand(port, request.src_b(), request.src_b_offset())?
                        .raw(),
                    destination_dma_address: operand(port, request.dst(), request.dst_offset())?
                        .raw(),
                    length: scalar(request.len())?,
                }),
            ),
            None,
        )),
    }
}

async fn broadcast<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    command_id: CommandId,
    request: &BroadcastCopyRequest,
) -> PdmaPortResult<(AbiCommand, Option<(A::Proposal, PdmaDmaWrite)>)> {
    let count = scalar(request.destinations().len())?;
    let extent = u64::from(count.value())
        .checked_mul(8)
        .ok_or(PortError::AllocationExhausted)?;
    let alignment = MemoryAlignment::try_new(64).map_err(|_| PortError::AllocationExhausted)?;
    let proposal = port
        .poll_provision(
            port.access
                .prepare_regions(&[PdmaRegionRequest::new(extent, alignment)]),
        )
        .await?;
    let [region] = proposal.regions() else {
        return Err(PortError::SubmissionState.into());
    };
    let region = *region;
    let mut bytes =
        vec![0_u8; usize::try_from(extent).map_err(|_| PortError::AllocationExhausted)?];
    for (destination, slot) in request.destinations().iter().zip(bytes.chunks_exact_mut(8)) {
        BroadcastDestinationEntry::from_address(operand(
            port,
            destination.dst(),
            destination.dst_offset(),
        )?)
        .encode(slot)
        .map_err(|_| PortError::SubmissionState)?;
    }
    let abi = AbiCommand::from_payload(
        abi_id(command_id.raw())?,
        AbiCommandPayloadValues::BroadcastCopy(AbiCommandPayloadBroadcastCopyValues {
            source_dma_address: operand(port, request.src(), request.src_offset())?.raw(),
            destination_list_dma_address: super::state::dma_address(region.base()).raw(),
            length: scalar(request.len())?,
            destination_count: count,
        }),
    );
    let write = PdmaDmaWrite::new(region.base(), bytes);
    Ok((abi, Some((proposal, write))))
}

fn operand<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    id: BufferId,
    offset: usize,
) -> PdmaPortResult<pdma_model_contract::abi::DmaAddress> {
    let address = super::state::checked_range(port, id, offset, 0)?;
    Ok(super::state::dma_address(address))
}

fn scalar(value: usize) -> PortResult<U32WordValues> {
    u32::try_from(value)
        .map(U32WordValues::new)
        .map_err(|_| PortError::SubmissionState)
}

fn abi_id(value: usize) -> PortResult<u64> {
    u64::try_from(value).map_err(|_| PortError::SubmissionState)
}
