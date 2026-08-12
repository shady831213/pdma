use super::resolve;
use crate::dma_space::DmaSpace;
use crate::{ModelError, ModelResult};
use pdma_model_contract::desc::{
    BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange, CopyCommand,
    CopyRequest, ExternalCommand, StridedCopyCommand, StridedCopyRequest, VectorAddCommand,
    VectorAddRequest, VectorBufferRange,
};
use pdma_model_contract::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiEvent, AbiFault, AbiOperandVisit, AbiStridedCopy,
    AbiVectorAdd, BroadcastDestinationEntry,
};
use pdma_model_contract::{BufferId, CommandId, CompletionRingId, DmaRangeStart, QueueId};

pub(super) fn resolve(
    dma_space: &DmaSpace,
    queue: QueueId,
    completion: CompletionRingId,
    command_id: CommandId,
    command: AbiCommand,
) -> ModelResult<Result<ExternalCommand, AbiEvent>> {
    let result = match command {
        AbiCommand::Copy(command) => copy(dma_space, queue, completion, command_id, command),
        AbiCommand::StridedCopy(command) => {
            strided(dma_space, queue, completion, command_id, command)
        }
        AbiCommand::BroadcastCopy(command) => {
            broadcast(dma_space, queue, completion, command_id, command)
        }
        AbiCommand::VectorAdd(command) => vector(dma_space, queue, completion, command_id, command),
    }?;
    Ok(result.map_err(|fault| AbiEvent::new(queue, command_id, fault)))
}

fn copy(
    dma: &DmaSpace,
    queue: QueueId,
    completion: CompletionRingId,
    command_id: CommandId,
    command: AbiCopy,
) -> ModelResult<Result<ExternalCommand, AbiFault>> {
    let length = usize::try_from(command.length()).map_err(|_| ModelError::DmaSubmissionState)?;
    let source = resolve::direct(
        dma,
        command.source(),
        u64::from(command.length()),
        AbiOperandVisit::CopySource,
    )?;
    let source = match source {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let destination = resolve::direct(
        dma,
        command.destination(),
        u64::from(command.length()),
        AbiOperandVisit::CopyDestination,
    )?;
    Ok(destination.map(|destination| {
        ExternalCommand::Copy(CopyCommand::new(
            command_id,
            queue,
            completion,
            CopyRequest::with_starts(source, destination, length),
        ))
    }))
}

fn strided(
    dma: &DmaSpace,
    queue: QueueId,
    completion: CompletionRingId,
    command_id: CommandId,
    command: AbiStridedCopy,
) -> ModelResult<Result<ExternalCommand, AbiFault>> {
    let mut starts = None;
    for row in 0..u64::from(command.rows()) {
        let source = resolve::strided(
            dma,
            command.source(),
            row,
            u64::from(command.source_stride()),
            u64::from(command.row_bytes()),
            AbiOperandVisit::StridedSource { row },
        )?;
        let source = match source {
            Ok(source) => source,
            Err(fault) => return Ok(Err(fault)),
        };
        let destination = resolve::strided(
            dma,
            command.destination(),
            row,
            u64::from(command.destination_stride()),
            u64::from(command.row_bytes()),
            AbiOperandVisit::StridedDestination { row },
        )?;
        let destination = match destination {
            Ok(destination) => destination,
            Err(fault) => return Ok(Err(fault)),
        };
        if starts.is_none() {
            starts = Some((source, destination));
        }
    }
    let (source, destination) = starts.unwrap_or((
        DmaRangeStart::new(BufferId::new(0), 0),
        DmaRangeStart::new(BufferId::new(0), 0),
    ));
    Ok(Ok(ExternalCommand::StridedCopy(StridedCopyCommand::new(
        command_id,
        queue,
        completion,
        StridedCopyRequest::with_starts(
            source,
            destination,
            usize::try_from(command.rows()).map_err(|_| ModelError::DmaSubmissionState)?,
            usize::try_from(command.row_bytes()).map_err(|_| ModelError::DmaSubmissionState)?,
            usize::try_from(command.source_stride()).map_err(|_| ModelError::DmaSubmissionState)?,
            usize::try_from(command.destination_stride())
                .map_err(|_| ModelError::DmaSubmissionState)?,
        ),
    ))))
}

fn broadcast(
    dma: &DmaSpace,
    queue: QueueId,
    completion: CompletionRingId,
    command_id: CommandId,
    command: AbiBroadcastCopy,
) -> ModelResult<Result<ExternalCommand, AbiFault>> {
    let length = usize::try_from(command.length()).map_err(|_| ModelError::DmaSubmissionState)?;
    let source = resolve::direct(
        dma,
        command.source(),
        u64::from(command.length()),
        AbiOperandVisit::BroadcastSource,
    )?;
    let source = match source {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let container = resolve::indirect_container(
        dma,
        command.destination_list(),
        u64::from(command.destination_count()),
    )?;
    let (_, extent) = match container {
        Ok(container) => container,
        Err(fault) => return Ok(Err(fault)),
    };
    let bytes = dma
        .read_iova(command.destination_list(), extent)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let mut destinations = Vec::new();
    destinations
        .try_reserve_exact(
            usize::try_from(command.destination_count())
                .map_err(|_| ModelError::DmaSubmissionState)?,
        )
        .map_err(|_| ModelError::DmaSubmissionState)?;
    for (index, entry) in bytes.chunks_exact(8).enumerate() {
        let address = BroadcastDestinationEntry::decode(entry)
            .map_err(|_| ModelError::DmaSubmissionState)?
            .address();
        let index = u64::try_from(index).map_err(|_| ModelError::DmaSubmissionState)?;
        let destination = resolve::direct(
            dma,
            address,
            u64::from(command.length()),
            AbiOperandVisit::BroadcastDestination { index },
        )?;
        let destination = match destination {
            Ok(destination) => destination,
            Err(fault) => return Ok(Err(fault)),
        };
        destinations.push(BroadcastDestinationRange::new(
            destination.object(),
            destination.offset(),
        ));
    }
    Ok(Ok(ExternalCommand::BroadcastCopy(
        BroadcastCopyCommand::new(
            command_id,
            queue,
            completion,
            BroadcastCopyRequest::new(source.object(), source.offset(), length, destinations),
        ),
    )))
}

fn vector(
    dma: &DmaSpace,
    queue: QueueId,
    completion: CompletionRingId,
    command_id: CommandId,
    command: AbiVectorAdd,
) -> ModelResult<Result<ExternalCommand, AbiFault>> {
    let length = usize::try_from(command.length()).map_err(|_| ModelError::DmaSubmissionState)?;
    let source_a = operand(
        dma,
        command.source_a(),
        command.length(),
        AbiOperandVisit::VectorSourceA,
    )?;
    let source_a = match source_a {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let source_b = operand(
        dma,
        command.source_b(),
        command.length(),
        AbiOperandVisit::VectorSourceB,
    )?;
    let source_b = match source_b {
        Ok(source) => source,
        Err(fault) => return Ok(Err(fault)),
    };
    let destination = operand(
        dma,
        command.destination(),
        command.length(),
        AbiOperandVisit::VectorDestination,
    )?;
    Ok(destination.map(|destination| {
        ExternalCommand::VectorAdd(VectorAddCommand::new(
            command_id,
            queue,
            completion,
            VectorAddRequest::new(
                vector_range(source_a),
                vector_range(source_b),
                vector_range(destination),
                length,
            ),
        ))
    }))
}

fn operand(
    dma: &DmaSpace,
    address: pdma_model_contract::abi::DmaAddress,
    length: u32,
    visit: AbiOperandVisit,
) -> ModelResult<Result<DmaRangeStart, AbiFault>> {
    resolve::direct(dma, address, u64::from(length), visit)
}

const fn vector_range(start: DmaRangeStart) -> VectorBufferRange {
    VectorBufferRange::new(start.object(), start.offset())
}
