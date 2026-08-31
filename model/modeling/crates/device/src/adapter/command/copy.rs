use super::{ResolutionContext, resolve};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiCommandPayloadCopyValues, AbiCommandPayloadStridedCopyValues, AbiFault, AbiOperandVisit,
    DmaAddress,
};
use pdma_model_contract::semantic::{Command, CommandOperation, CopyRequest, StridedCopyRequest};

pub(super) fn copy(
    context: ResolutionContext<'_>,
    command: AbiCommandPayloadCopyValues,
) -> ModelResult<Result<Command, AbiFault>> {
    let length =
        usize::try_from(command.length.value()).map_err(|_| ModelError::DmaSubmissionState)?;
    let source = resolve::direct(
        context.dma,
        DmaAddress::new(command.source_dma_address),
        u64::from(command.length.value()),
        AbiOperandVisit::CopySource,
    )?;
    let source = match source {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let destination = resolve::direct(
        context.dma,
        DmaAddress::new(command.destination_dma_address),
        u64::from(command.length.value()),
        AbiOperandVisit::CopyDestination,
    )?;
    Ok(destination.map(|destination| {
        context.complete(CommandOperation::Copy(CopyRequest::with_starts(
            source,
            destination,
            length,
        )))
    }))
}

pub(super) fn strided(
    context: ResolutionContext<'_>,
    command: AbiCommandPayloadStridedCopyValues,
) -> ModelResult<Result<Command, AbiFault>> {
    let row_count =
        usize::try_from(command.rows.value()).map_err(|_| ModelError::DmaSubmissionState)?;
    if row_count == 0 {
        return Ok(Ok(context.complete(CommandOperation::StridedCopy(
            StridedCopyRequest::empty(
                scalar(command.row_bytes.value())?,
                scalar(command.source_stride.value())?,
                scalar(command.destination_stride.value())?,
            ),
        ))));
    }
    let source = row_source(context, command, 0)?;
    let source = match source {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let destination = row_destination(context, command, 0)?;
    let destination = match destination {
        Ok(destination) => destination,
        Err(fault) => return Ok(Err(fault)),
    };
    for row in 1..u64::from(command.rows.value()) {
        if let Err(fault) = row_source(context, command, row)? {
            return Ok(Err(fault));
        }
        if let Err(fault) = row_destination(context, command, row)? {
            return Ok(Err(fault));
        }
    }
    Ok(Ok(context.complete(CommandOperation::StridedCopy(
        StridedCopyRequest::with_starts(
            source,
            destination,
            row_count,
            scalar(command.row_bytes.value())?,
            scalar(command.source_stride.value())?,
            scalar(command.destination_stride.value())?,
        ),
    ))))
}

fn row_source(
    context: ResolutionContext<'_>,
    command: AbiCommandPayloadStridedCopyValues,
    row: u64,
) -> ModelResult<Result<pdma_model_contract::semantic::DmaRangeStart, AbiFault>> {
    resolve::strided(
        context.dma,
        DmaAddress::new(command.source_dma_address),
        row,
        u64::from(command.source_stride.value()),
        u64::from(command.row_bytes.value()),
        AbiOperandVisit::StridedSource { row },
    )
}

fn row_destination(
    context: ResolutionContext<'_>,
    command: AbiCommandPayloadStridedCopyValues,
    row: u64,
) -> ModelResult<Result<pdma_model_contract::semantic::DmaRangeStart, AbiFault>> {
    resolve::strided(
        context.dma,
        DmaAddress::new(command.destination_dma_address),
        row,
        u64::from(command.destination_stride.value()),
        u64::from(command.row_bytes.value()),
        AbiOperandVisit::StridedDestination { row },
    )
}

fn scalar(value: u32) -> ModelResult<usize> {
    usize::try_from(value).map_err(|_| ModelError::DmaSubmissionState)
}
