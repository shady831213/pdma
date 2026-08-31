use super::{ResolutionContext, resolve};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiCommandPayloadBroadcastCopyValues, AbiFault, AbiOperandVisit, BroadcastDestinationEntry,
    DmaAddress,
};
use pdma_model_contract::semantic::{
    BroadcastCopyRequest, BroadcastDestinationRange, Command, CommandOperation,
};

pub(super) fn broadcast(
    context: ResolutionContext<'_>,
    command: AbiCommandPayloadBroadcastCopyValues,
) -> ModelResult<Result<Command, AbiFault>> {
    let length =
        usize::try_from(command.length.value()).map_err(|_| ModelError::DmaSubmissionState)?;
    let source = resolve::direct(
        context.dma,
        DmaAddress::new(command.source_dma_address),
        u64::from(command.length.value()),
        AbiOperandVisit::BroadcastSource,
    )?;
    let source = match source {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let container = resolve::indirect_container(
        context.dma,
        DmaAddress::new(command.destination_list_dma_address),
        u64::from(command.destination_count.value()),
    )?;
    let (_, extent) = match container {
        Ok(container) => container,
        Err(fault) => return Ok(Err(fault)),
    };
    let bytes = context
        .dma
        .read_memory(
            DmaAddress::new(command.destination_list_dma_address),
            extent,
        )
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let mut destinations = Vec::new();
    destinations
        .try_reserve_exact(
            usize::try_from(command.destination_count.value())
                .map_err(|_| ModelError::DmaSubmissionState)?,
        )
        .map_err(|_| ModelError::DmaSubmissionState)?;
    for (index, entry) in bytes.chunks_exact(8).enumerate() {
        let address = BroadcastDestinationEntry::decode(entry)
            .map_err(|_| ModelError::DmaSubmissionState)?
            .address();
        let index = u64::try_from(index).map_err(|_| ModelError::DmaSubmissionState)?;
        let destination = resolve::direct(
            context.dma,
            address,
            u64::from(command.length.value()),
            AbiOperandVisit::BroadcastDestination { index },
        )?;
        match destination {
            Ok(destination) => destinations.push(BroadcastDestinationRange::new(
                destination.object(),
                destination.offset(),
            )),
            Err(fault) => return Ok(Err(fault)),
        }
    }
    Ok(Ok(context.complete(CommandOperation::BroadcastCopy(
        BroadcastCopyRequest::new(source.object(), source.offset(), length, destinations),
    ))))
}
