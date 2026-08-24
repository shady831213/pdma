use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::register::{PdmaDmaWrite, PdmaPreparedProposal, PdmaRegionRequest, PdmaSubmissionBatch};
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiId, AbiStridedCopy, AbiVectorAdd,
    BroadcastDestinationEntry, BroadcastDmaAddresses, BroadcastShape, CopyDmaAddresses,
    CopyStrides, RingRecord, StridedCopyShape, VectorDmaAddresses,
};
use pdma_model_contract::semantic::{BufferId, ExternalCommand, QueueConfigHandle};

pub(super) async fn submit<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    handle: QueueConfigHandle,
    command: ExternalCommand,
) -> PdmaPortResult<()> {
    let queue = refresh(port, handle).await?;
    let (abi, indirect) = encode_command(port, command).await?;
    let mut descriptor = vec![0_u8; usize::from(RingRecord::Submission.record_size())];
    abi.encode(&mut descriptor)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let producer = queue.status.submission().producer();
    let slot_offset = queue
        .rings
        .submission()
        .slot(producer)
        .checked_mul(u64::from(RingRecord::Submission.record_size()))
        .ok_or(ModelError::DmaSubmissionState)?;
    let address = queue
        .rings
        .submission()
        .region()
        .abi_base()
        .checked_add(slot_offset)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let descriptor = PdmaDmaWrite::new(MemoryAddress::new(address.raw()), descriptor);
    let request = PdmaSubmissionBatch::new(queue.status.queue(), producer, descriptor, indirect);
    let bytes = port.poll_batch(port.access.submit_queue(request)).await?;
    let status = super::control::decode_status(&bytes, queue.rings)?;
    Ok(port.with_state(|state| -> ModelResult<()> {
        let active = state.queue(handle)?;
        if active.status.submission().producer() != producer {
            return Err(ModelError::DmaSubmissionState);
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
    Ok(port.with_state(|state| -> ModelResult<()> {
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
    command: ExternalCommand,
) -> PdmaPortResult<(AbiCommand, Option<(A::Proposal, PdmaDmaWrite)>)> {
    match command {
        ExternalCommand::Copy(command) => {
            let request = command.request();
            Ok((
                AbiCommand::Copy(AbiCopy::new(
                    abi_id(command.command_id().raw())?,
                    CopyDmaAddresses::new(
                        operand(port, request.src(), request.src_start().offset())?,
                        operand(port, request.dst(), request.dst_start().offset())?,
                    ),
                    scalar(request.len())?,
                )),
                None,
            ))
        }
        ExternalCommand::StridedCopy(command) => {
            let request = command.request();
            let shape = StridedCopyShape::new(
                scalar(request.rows())?,
                scalar(request.row_bytes())?,
                CopyStrides::new(scalar(request.src_stride())?, scalar(request.dst_stride())?),
            );
            Ok((
                AbiCommand::StridedCopy(AbiStridedCopy::new(
                    abi_id(command.command_id().raw())?,
                    CopyDmaAddresses::new(
                        operand(port, request.src(), request.src_start().offset())?,
                        operand(port, request.dst(), request.dst_start().offset())?,
                    ),
                    shape,
                )),
                None,
            ))
        }
        ExternalCommand::BroadcastCopy(command) => broadcast(port, command).await,
        ExternalCommand::VectorAdd(command) => {
            let request = command.request();
            Ok((
                AbiCommand::VectorAdd(AbiVectorAdd::new(
                    abi_id(command.command_id().raw())?,
                    VectorDmaAddresses::new(
                        operand(port, request.src_a(), request.src_a_offset())?,
                        operand(port, request.src_b(), request.src_b_offset())?,
                        operand(port, request.dst(), request.dst_offset())?,
                    ),
                    scalar(request.len())?,
                )),
                None,
            ))
        }
    }
}

async fn broadcast<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    command: pdma_model_contract::semantic::BroadcastCopyCommand,
) -> PdmaPortResult<(AbiCommand, Option<(A::Proposal, PdmaDmaWrite)>)> {
    let request = command.request();
    let count = scalar(request.destinations().len())?;
    let extent = u64::from(count)
        .checked_mul(8)
        .ok_or(ModelError::DmaAllocationExhausted)?;
    let alignment = MemoryAlignment::try_new(64).map_err(|_| ModelError::DmaAllocationExhausted)?;
    let proposal = port
        .poll_provision(
            port.access
                .prepare_regions(&[PdmaRegionRequest::new(extent, alignment)]),
        )
        .await?;
    let [region] = proposal.regions() else {
        return Err(ModelError::DmaSubmissionState.into());
    };
    let region = *region;
    let mut bytes =
        vec![0_u8; usize::try_from(extent).map_err(|_| ModelError::DmaAllocationExhausted)?];
    for (destination, slot) in request.destinations().iter().zip(bytes.chunks_exact_mut(8)) {
        BroadcastDestinationEntry::new(operand(port, destination.dst(), destination.dst_offset())?)
            .encode(slot)
            .map_err(|_| ModelError::DmaSubmissionState)?;
    }
    let abi = AbiBroadcastCopy::try_new(
        abi_id(command.command_id().raw())?,
        BroadcastDmaAddresses::new(
            operand(port, request.src(), request.src_offset())?,
            super::state::dma_address(region.base()),
        ),
        BroadcastShape::new(scalar(request.len())?, count),
    )
    .map_err(|_| ModelError::DmaSubmissionState)?;
    let write = PdmaDmaWrite::new(region.base(), bytes);
    Ok((AbiCommand::BroadcastCopy(abi), Some((proposal, write))))
}

fn operand<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    id: BufferId,
    offset: usize,
) -> PdmaPortResult<pdma_model_contract::abi::DmaAddress> {
    let address = super::state::checked_range(port, id, offset, 0)?;
    Ok(super::state::dma_address(address))
}

fn scalar(value: usize) -> ModelResult<u32> {
    u32::try_from(value).map_err(|_| ModelError::DmaSubmissionState)
}

fn abi_id(value: usize) -> ModelResult<AbiId> {
    AbiId::try_from(value).map_err(|_| ModelError::DmaSubmissionState)
}
