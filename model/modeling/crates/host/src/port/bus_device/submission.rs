use super::{AsyncPdmaBus, BusDevicePort, BusDmaErrorAdapter};
use crate::ModelError;
use crate::ModelResult;
use crate::{BusDmaStage, BusDmaWrite, BusSubmission, QueueControlWrite};
use pdma_model_contract::{
    AbiBroadcastCopy, AbiCommand, AbiCopy, AbiId, AbiStridedCopy, AbiVectorAdd,
    BroadcastDestinationEntry, BroadcastIovas, BroadcastShape, CopyIovas, CopyStrides, DmaAddress,
    QueueCursorUpdate, RingRecord, StridedCopyShape, VectorIovas,
};
use pdma_model_contract::{BufferId, ExternalCommand, QueueConfigHandle};

pub(super) async fn submit<B>(
    port: &BusDevicePort<B>,
    handle: QueueConfigHandle,
    command: ExternalCommand,
) -> ModelResult<()>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
    B::ControlError: super::BusControlErrorAdapter,
{
    let queue = port.state.with(|state| state.queue(handle))??;
    let observed = port
        .poll_control(
            port.bus.read_queue_status(queue.status.queue()),
            queue.status.queue(),
        )
        .await?;
    port.state.with(|state| {
        let current = state.queue(handle)?;
        state.queue = Some(super::state::QueueState {
            status: observed,
            ..current
        });
        Ok::<(), ModelError>(())
    })??;
    let queue = port.state.with(|state| state.queue(handle))??;
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
    let status = port
        .poll_control(
            port.bus.accept_submission(BusSubmission::new(
                queue.status.queue(),
                producer,
                BusDmaWrite::new(address, descriptor),
                indirect,
            )),
            queue.status.queue(),
        )
        .await?;
    port.state.with(|state| {
        let active = state.queue(handle)?;
        if active.status.submission().producer() != producer {
            return Err(ModelError::DmaSubmissionState);
        }
        state.queue = Some(super::state::QueueState { status, ..active });
        Ok::<(), ModelError>(())
    })?
}

pub(super) async fn doorbell<B>(
    port: &BusDevicePort<B>,
    handle: QueueConfigHandle,
) -> ModelResult<()>
where
    B: AsyncPdmaBus,
    B::ControlError: super::BusControlErrorAdapter,
{
    let queue = port.state.with(|state| state.queue(handle))??;
    let observed = port
        .poll_control(
            port.bus.read_queue_status(queue.status.queue()),
            queue.status.queue(),
        )
        .await?;
    port.state.with(|state| {
        let current = state.queue(handle)?;
        state.queue = Some(super::state::QueueState {
            status: observed,
            ..current
        });
        Ok::<(), ModelError>(())
    })??;
    let queue = port.state.with(|state| state.queue(handle))??;
    let status = port
        .poll_control(
            port.bus
                .write_queue_control(QueueControlWrite::CaptureSubmissionProducer(
                    QueueCursorUpdate::new(
                        queue.status.queue(),
                        queue.status.submission().producer(),
                    ),
                )),
            queue.status.queue(),
        )
        .await?;
    port.state.with(|state| {
        let active = state.queue(handle)?;
        state.queue = Some(super::state::QueueState { status, ..active });
        Ok(())
    })?
}

async fn encode_command<B>(
    port: &BusDevicePort<B>,
    command: ExternalCommand,
) -> ModelResult<(AbiCommand, Option<(B::Stage, BusDmaWrite)>)>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
{
    match command {
        ExternalCommand::Copy(command) => {
            let request = command.request();
            let iovas = CopyIovas::new(
                operand(port, request.src(), request.src_start().offset())?,
                operand(port, request.dst(), request.dst_start().offset())?,
            );
            Ok((
                AbiCommand::Copy(AbiCopy::new(
                    abi_id(command.command_id().raw())?,
                    iovas,
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
                    CopyIovas::new(
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
                    VectorIovas::new(
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

async fn broadcast<B>(
    port: &BusDevicePort<B>,
    command: pdma_model_contract::BroadcastCopyCommand,
) -> ModelResult<(AbiCommand, Option<(B::Stage, BusDmaWrite)>)>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
{
    let request = command.request();
    let count = scalar(request.destinations().len())?;
    let extent = u64::from(count)
        .checked_mul(8)
        .ok_or(ModelError::DmaAllocationExhausted)?;
    let stage = port
        .poll_dma(port.bus.stage_memory_regions(&[extent]), None)
        .await?;
    let [region] = stage.proposed_regions() else {
        return Err(ModelError::DmaSubmissionState);
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
        BroadcastIovas::new(
            operand(port, request.src(), request.src_offset())?,
            region.abi_base(),
        ),
        BroadcastShape::new(scalar(request.len())?, count),
    )
    .map_err(|_| ModelError::DmaSubmissionState)?;
    Ok((
        AbiCommand::BroadcastCopy(abi),
        Some((stage, BusDmaWrite::new(region.abi_base(), bytes))),
    ))
}

fn operand<B>(port: &BusDevicePort<B>, id: BufferId, offset: usize) -> ModelResult<DmaAddress> {
    port.state.with(|state| {
        let region = state.region(id)?;
        let offset = u64::try_from(offset).map_err(|_| ModelError::RangeOverflow)?;
        if offset > region.length() {
            return Err(ModelError::OutOfBounds {
                buffer: id,
                len: usize::try_from(region.length()).map_err(|_| ModelError::RangeOverflow)?,
                end: usize::try_from(offset).map_err(|_| ModelError::RangeOverflow)?,
            });
        }
        region
            .abi_base()
            .checked_add(offset)
            .map_err(|_| ModelError::RangeOverflow)
    })?
}

fn scalar(value: usize) -> ModelResult<u32> {
    u32::try_from(value).map_err(|_| ModelError::DmaSubmissionState)
}

fn abi_id(value: usize) -> ModelResult<AbiId> {
    AbiId::try_from(value).map_err(|_| ModelError::DmaSubmissionState)
}
