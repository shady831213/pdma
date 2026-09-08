use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::adapter::abi::PdmaRegionRequest;
use crate::{BroadcastCopyRequest, Command, CommandOperation};
use crate::{PortError, PortResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues, AbiCommandPayloadVectorAddValues,
    BroadcastDestinationEntry, U32WordValues,
};
use pdma_model_contract::semantic::{BufferId, CommandId, DmaRangeStart};

pub(super) async fn encode_command<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    command: Command,
) -> PdmaPortResult<AbiCommand> {
    let command_id = command.command_id();
    match command.into_operation() {
        CommandOperation::Copy(request) => Ok(AbiCommand::from_payload(
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
        )),
        CommandOperation::StridedCopy(request) => Ok(AbiCommand::from_payload(
            abi_id(command_id.raw())?,
            AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
                source_dma_address: optional_operand(port, request.src_start())?.raw(),
                destination_dma_address: optional_operand(port, request.dst_start())?.raw(),
                rows: scalar(request.rows())?,
                row_bytes: scalar(request.row_bytes())?,
                source_stride: scalar(request.src_stride())?,
                destination_stride: scalar(request.dst_stride())?,
            }),
        )),
        CommandOperation::BroadcastCopy(request) => broadcast(port, command_id, &request).await,
        CommandOperation::VectorAdd(request) => Ok(AbiCommand::from_payload(
            abi_id(command_id.raw())?,
            AbiCommandPayloadValues::VectorAdd(AbiCommandPayloadVectorAddValues {
                source_a_dma_address: operand(port, request.src_a(), request.src_a_offset())?.raw(),
                source_b_dma_address: operand(port, request.src_b(), request.src_b_offset())?.raw(),
                destination_dma_address: operand(port, request.dst(), request.dst_offset())?.raw(),
                length: scalar(request.len())?,
            }),
        )),
    }
}

async fn broadcast<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    command_id: CommandId,
    request: &BroadcastCopyRequest,
) -> PdmaPortResult<AbiCommand> {
    let count = scalar(request.destinations().len())?;
    let extent = u64::from(count.value())
        .checked_mul(8)
        .ok_or(PortError::AllocationExhausted)?;
    let alignment = MemoryAlignment::try_new(64).map_err(|_| PortError::AllocationExhausted)?;
    let region = port
        .poll_provision(
            port.access
                .allocate_region(PdmaRegionRequest::new(extent, alignment)),
        )
        .await?;
    port.with_state(|state| state.regions.push(region))?;
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
    port.validate_owner()?;
    MemoryAccess::write(&port.access, region.base(), MemoryAlignment::ONE, &bytes)?;
    Ok(abi)
}

fn operand<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    id: BufferId,
    offset: usize,
) -> PdmaPortResult<pdma_model_contract::abi::DmaAddress> {
    let base = port.with_state(|state| state.region(id).map(|region| region.base().raw()))?;
    let base = match base {
        Ok(base) => base,
        Err(PortError::UnknownBuffer(_)) => 0,
        Err(error) => return Err(error.into()),
    };
    let offset = u64::try_from(offset).map_err(|_| PortError::RangeOverflow)?;
    Ok(pdma_model_contract::abi::DmaAddress::new(
        base.wrapping_add(offset),
    ))
}

fn optional_operand<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    start: Option<DmaRangeStart>,
) -> PdmaPortResult<pdma_model_contract::abi::DmaAddress> {
    start.map_or_else(
        || Ok(pdma_model_contract::abi::DmaAddress::new(0)),
        |start| operand(port, start.object(), start.offset()),
    )
}

fn scalar(value: usize) -> PortResult<U32WordValues> {
    u32::try_from(value)
        .map(U32WordValues::new)
        .map_err(|_| PortError::SubmissionState)
}

fn abi_id(value: usize) -> PortResult<u64> {
    u64::try_from(value).map_err(|_| PortError::SubmissionState)
}
