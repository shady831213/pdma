use super::super::resolve;
use super::ResolutionContext;
use crate::runner::execution::command::{ExecutionCommand, ExecutionOperation, VectorAddOperation};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiCommandPayloadVectorAddValues, AbiFault, AbiOperandVisit, DmaAddress,
};

pub(super) fn vector(
    context: ResolutionContext<'_>,
    command: AbiCommandPayloadVectorAddValues,
) -> ModelResult<Result<ExecutionCommand, AbiFault>> {
    let length =
        usize::try_from(command.length.value()).map_err(|_| ModelError::DmaSubmissionState)?;
    let source_a = operand(
        context,
        DmaAddress::new(command.source_a_dma_address),
        command.length.value(),
        AbiOperandVisit::VectorSourceA,
    )?;
    let source_a = match source_a {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let source_b = operand(
        context,
        DmaAddress::new(command.source_b_dma_address),
        command.length.value(),
        AbiOperandVisit::VectorSourceB,
    )?;
    let source_b = match source_b {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let destination = operand(
        context,
        DmaAddress::new(command.destination_dma_address),
        command.length.value(),
        AbiOperandVisit::VectorDestination,
    )?;
    Ok(destination.map(|destination| {
        context.complete(ExecutionOperation::VectorAdd(VectorAddOperation {
            source_a,
            source_b,
            destination,
            length,
        }))
    }))
}

fn operand(
    context: ResolutionContext<'_>,
    address: pdma_model_contract::abi::DmaAddress,
    length: u32,
    visit: AbiOperandVisit,
) -> ModelResult<Result<pdma_model_contract::semantic::DmaRangeStart, AbiFault>> {
    resolve::direct(context.dma, address, u64::from(length), visit)
}
