use super::{AsyncPdmaBus, BusControlErrorAdapter, BusDevicePort, BusDmaErrorAdapter};
use crate::QueueControlWrite;
use pdma_model_contract::abi::{
    AbiEventKind, DmaAddress, QueueCursorUpdate, RingCursor, decode_completion, decode_event,
};
use pdma_model_contract::{
    BufferId, CompletionRecord, EpochIrqSource, EventErrorKind, EventRecord, IrqRoute, ModelError,
    ModelResult, QueueConfigHandle,
};
use std::{future::poll_fn, task::Poll};

mod irq_control;

pub(super) async fn completion<B>(
    port: &BusDevicePort<B>,
    handle: QueueConfigHandle,
) -> ModelResult<Option<CompletionRecord>>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
    B::ControlError: super::BusControlErrorAdapter,
{
    let queue = refresh(port, handle).await?;
    let cursors = queue.status.completion();
    if cursors.producer() == cursors.consumer() {
        return Ok(None);
    }
    let address = slot_address(queue.rings.completion(), cursors.consumer())?;
    let bytes = port
        .poll_dma(
            port.bus.read_dma(
                address,
                queue.rings.completion().extent() / u64::from(queue.rings.completion().depth()),
            ),
            None,
        )
        .await?;
    let record = decode_completion(&bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let next = cursors
        .consumer()
        .checked_increment(1)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    commit_consumer(
        port,
        handle,
        queue.status.queue(),
        cursors.consumer(),
        QueueControlWrite::PublishCompletionConsumer(QueueCursorUpdate::new(
            queue.status.queue(),
            next,
        )),
    )
    .await?;
    Ok(Some(record))
}

pub(super) async fn event<B>(
    port: &BusDevicePort<B>,
    handle: QueueConfigHandle,
) -> ModelResult<Option<EventRecord>>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
    B::ControlError: super::BusControlErrorAdapter,
{
    let queue = refresh(port, handle).await?;
    let cursors = queue.status.event();
    if cursors.producer() == cursors.consumer() {
        return Ok(None);
    }
    let address = slot_address(queue.rings.event(), cursors.consumer())?;
    let bytes = port
        .poll_dma(
            port.bus.read_dma(
                address,
                queue.rings.event().extent() / u64::from(queue.rings.event().depth()),
            ),
            None,
        )
        .await?;
    let decoded = decode_event(&bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let record = semantic_event(port, decoded)?;
    let next = cursors
        .consumer()
        .checked_increment(1)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    commit_consumer(
        port,
        handle,
        queue.status.queue(),
        cursors.consumer(),
        QueueControlWrite::PublishEventConsumer(QueueCursorUpdate::new(queue.status.queue(), next)),
    )
    .await?;
    Ok(Some(record))
}

async fn refresh<B>(
    port: &BusDevicePort<B>,
    handle: QueueConfigHandle,
) -> ModelResult<super::state::QueueState>
where
    B: AsyncPdmaBus,
    B::ControlError: super::BusControlErrorAdapter,
{
    let queue = port.state.with(|state| state.queue(handle))??;
    let status = port
        .poll_control(
            port.bus.read_queue_status(queue.status.queue()),
            queue.status.queue(),
        )
        .await?;
    port.state.with(|state| {
        let current = state.queue(handle)?;
        let refreshed = super::state::QueueState { status, ..current };
        state.queue = Some(refreshed);
        Ok(refreshed)
    })?
}

async fn commit_consumer<B>(
    port: &BusDevicePort<B>,
    handle: QueueConfigHandle,
    queue: pdma_model_contract::abi::EpochQueue,
    expected_consumer: RingCursor,
    request: QueueControlWrite,
) -> ModelResult<()>
where
    B: AsyncPdmaBus,
    B::ControlError: super::BusControlErrorAdapter,
{
    let mut raw = Box::pin(port.bus.write_queue_control(request));
    let status = poll_fn(|context| {
        if let Err(error) = port.state.with(|_| ()) {
            return Poll::Ready(Err(error));
        }
        let observed = match port.state.with(|state| {
            let status = state.queue(handle)?.status;
            match request {
                QueueControlWrite::PublishCompletionConsumer(_) => {
                    Ok(status.completion().consumer())
                }
                QueueControlWrite::PublishEventConsumer(_) => Ok(status.event().consumer()),
                QueueControlWrite::CaptureSubmissionProducer(_)
                | QueueControlWrite::PublishSubmissionConsumer(_)
                | QueueControlWrite::PublishCompletionProducer(_)
                | QueueControlWrite::PublishEventProducer(_) => Err(ModelError::DmaSubmissionState),
            }
        }) {
            Ok(Ok(observed)) => observed,
            Ok(Err(error)) | Err(error) => return Poll::Ready(Err(error)),
        };
        if observed != expected_consumer {
            return Poll::Ready(Err(ModelError::DmaSubmissionState));
        }
        match raw.as_mut().poll(context) {
            Poll::Ready(result) => {
                Poll::Ready(result.map_err(|error| error.into_model_error(queue)))
            }
            Poll::Pending => Poll::Pending,
        }
    })
    .await?;
    port.state.with(|state| {
        let current = state.queue(handle)?;
        state.queue = Some(super::state::QueueState { status, ..current });
        Ok(())
    })?
}

fn slot_address(
    layout: pdma_model_contract::abi::RingLayout,
    cursor: RingCursor,
) -> ModelResult<DmaAddress> {
    let offset = layout
        .slot(cursor)
        .checked_mul(u64::from(layout.record().record_size()))
        .ok_or(ModelError::DmaSubmissionState)?;
    layout
        .region()
        .base()
        .checked_add(offset)
        .map_err(|_| ModelError::DmaSubmissionState)
}

fn semantic_event<B>(
    port: &BusDevicePort<B>,
    event: pdma_model_contract::abi::AbiEvent,
) -> ModelResult<EventRecord> {
    let (kind, locator, detail0, detail1, _, _, _) = event.fault().tuple();
    let locator = DmaAddress::new(locator);
    let buffer = port.state.with(|state| {
        state
            .regions
            .iter()
            .find(|region| region.base() <= locator && locator < region.exclusive_end())
            .map_or(BufferId::new(usize::MAX), |region| region.object())
    })?;
    let error_kind = match kind {
        AbiEventKind::UnmappedIova => EventErrorKind::UnknownBuffer,
        AbiEventKind::RangeOverflow
        | AbiEventKind::InvalidVersion
        | AbiEventKind::InvalidSize
        | AbiEventKind::InvalidOpcode
        | AbiEventKind::InvalidFlags
        | AbiEventKind::InvalidReserved
        | AbiEventKind::InvalidIndirectCount
        | AbiEventKind::InvalidIndirectAlignment
        | AbiEventKind::ValueOutOfRange => EventErrorKind::RangeOverflow,
        AbiEventKind::OutOfBounds => EventErrorKind::OutOfBounds {
            len: usize::try_from(detail0).map_err(|_| ModelError::RangeOverflow)?,
            end: usize::try_from(detail1).map_err(|_| ModelError::RangeOverflow)?,
        },
    };
    Ok(EventRecord::new(
        event.queue_id(),
        event.command_id(),
        error_kind,
        buffer,
    ))
}

pub(super) use irq_control::{ack, configure, disable, enable, mask, unmask};
