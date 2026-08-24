use super::super::resolve;
use super::ResolutionContext;
use crate::runner::native::execution::{
    CopyOperation, ExecutionCommand, ExecutionOperation, ResolvedStridedRows, StridedCopyOperation,
};
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiFault, AbiOperandVisit, DescriptorSchemaPayloadCopyValues,
    DescriptorSchemaPayloadStridedCopyValues, DmaAddress,
};
use std::num::NonZeroUsize;

pub(super) fn copy(
    context: ResolutionContext<'_>,
    command: DescriptorSchemaPayloadCopyValues,
) -> ModelResult<Result<ExecutionCommand, AbiFault>> {
    let length = usize::try_from(command.length).map_err(|_| ModelError::DmaSubmissionState)?;
    let source = resolve::direct(
        context.dma,
        DmaAddress::new(command.source_dma_address),
        u64::from(command.length),
        AbiOperandVisit::CopySource,
    )?;
    let source = match source {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let destination = resolve::direct(
        context.dma,
        DmaAddress::new(command.destination_dma_address),
        u64::from(command.length),
        AbiOperandVisit::CopyDestination,
    )?;
    Ok(destination.map(|destination| {
        context.complete(ExecutionOperation::Copy(CopyOperation {
            source,
            destination,
            length,
        }))
    }))
}

pub(super) fn strided(
    context: ResolutionContext<'_>,
    command: DescriptorSchemaPayloadStridedCopyValues,
) -> ModelResult<Result<ExecutionCommand, AbiFault>> {
    let row_count = usize::try_from(command.rows).map_err(|_| ModelError::DmaSubmissionState)?;
    let Some(count) = NonZeroUsize::new(row_count) else {
        return Ok(Ok(context.complete(ExecutionOperation::StridedCopy(
            StridedCopyOperation {
                rows: ResolvedStridedRows::Empty,
                row_bytes: scalar(command.row_bytes)?,
                source_stride: scalar(command.source_stride)?,
                destination_stride: scalar(command.destination_stride)?,
            },
        ))));
    };
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
    for row in 1..u64::from(command.rows) {
        if let Err(fault) = row_source(context, command, row)? {
            return Ok(Err(fault));
        }
        if let Err(fault) = row_destination(context, command, row)? {
            return Ok(Err(fault));
        }
    }
    Ok(Ok(context.complete(ExecutionOperation::StridedCopy(
        StridedCopyOperation {
            rows: ResolvedStridedRows::NonEmpty {
                source,
                destination,
                count,
            },
            row_bytes: scalar(command.row_bytes)?,
            source_stride: scalar(command.source_stride)?,
            destination_stride: scalar(command.destination_stride)?,
        },
    ))))
}

fn row_source(
    context: ResolutionContext<'_>,
    command: DescriptorSchemaPayloadStridedCopyValues,
    row: u64,
) -> ModelResult<Result<pdma_model_contract::semantic::DmaRangeStart, AbiFault>> {
    resolve::strided(
        context.dma,
        DmaAddress::new(command.source_dma_address),
        row,
        u64::from(command.source_stride),
        u64::from(command.row_bytes),
        AbiOperandVisit::StridedSource { row },
    )
}

fn row_destination(
    context: ResolutionContext<'_>,
    command: DescriptorSchemaPayloadStridedCopyValues,
    row: u64,
) -> ModelResult<Result<pdma_model_contract::semantic::DmaRangeStart, AbiFault>> {
    resolve::strided(
        context.dma,
        DmaAddress::new(command.destination_dma_address),
        row,
        u64::from(command.destination_stride),
        u64::from(command.row_bytes),
        AbiOperandVisit::StridedDestination { row },
    )
}

fn scalar(value: u32) -> ModelResult<usize> {
    usize::try_from(value).map_err(|_| ModelError::DmaSubmissionState)
}
