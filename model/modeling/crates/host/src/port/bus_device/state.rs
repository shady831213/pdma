use super::{AsyncPdmaBus, BusDevicePort, BusDmaErrorAdapter};
use crate::BusDmaStage;
use pdma_model_contract::abi::{
    DeviceEpoch, DmaAddress, DmaRegion, QueueActivation, QueueRingRegions, RingLayout, RingRecord,
};
use pdma_model_contract::{
    BufferId, CompletionRingId, ModelError, ModelResult, QueueConfig, QueueConfigHandle,
    QueueDirectoryEntry, QueueId,
};

#[derive(Debug, Clone)]
pub(super) struct PortState {
    pub(super) epoch: DeviceEpoch,
    pub(super) queue: Option<QueueState>,
    pub(super) regions: Vec<DmaRegion>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct QueueState {
    pub(super) handle: QueueConfigHandle,
    pub(super) rings: QueueRingRegions,
    pub(super) status: crate::BusQueueStatus,
}

impl PortState {
    pub(super) const fn new(epoch: DeviceEpoch) -> Self {
        Self {
            epoch,
            queue: None,
            regions: Vec::new(),
        }
    }

    pub(super) fn queue(&self, handle: QueueConfigHandle) -> ModelResult<QueueState> {
        let queue = self
            .queue
            .ok_or_else(|| ModelError::UnknownQueue(handle.queue_id()))?;
        if queue.handle != handle {
            return Err(if queue.handle.epoch() == handle.epoch() {
                ModelError::UnknownQueue(handle.queue_id())
            } else {
                ModelError::StaleDeviceEpoch {
                    expected: queue.handle.epoch(),
                    actual: handle.epoch(),
                }
            });
        }
        Ok(queue)
    }

    pub(super) fn region(&self, id: BufferId) -> ModelResult<DmaRegion> {
        self.regions
            .iter()
            .copied()
            .find(|region| region.object() == id)
            .ok_or(ModelError::UnknownBuffer(id))
    }
}

pub(super) async fn create_queue<B>(
    port: &BusDevicePort<B>,
    config: QueueConfig,
) -> ModelResult<QueueConfigHandle>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
    B::ControlError: super::BusControlErrorAdapter,
{
    let epoch = port.state.with(|state| {
        if state.queue.is_some() {
            Err(ModelError::DmaSubmissionState)
        } else {
            Ok(state.epoch)
        }
    })??;
    let lengths = ring_lengths(config)?;
    let stage = port
        .poll_dma(port.bus.stage_dma_regions(&lengths), None)
        .await?;
    let [submission, completion, event] = stage.proposed_regions() else {
        return Err(ModelError::DmaSubmissionState);
    };
    let proposed = [*submission, *completion, *event];
    let [submission, completion, event] = proposed;
    let rings = QueueRingRegions::new(
        RingLayout::try_new(
            submission,
            lengths[0] / u64::from(RingRecord::Submission.record_size()),
            RingRecord::Submission,
        )
        .map_err(|_| ModelError::DmaSubmissionState)?,
        RingLayout::try_new(
            completion,
            lengths[1] / u64::from(RingRecord::Completion.record_size()),
            RingRecord::Completion,
        )
        .map_err(|_| ModelError::DmaSubmissionState)?,
        RingLayout::try_new(
            event,
            lengths[2] / u64::from(RingRecord::Event.record_size()),
            RingRecord::Event,
        )
        .map_err(|_| ModelError::DmaSubmissionState)?,
    );
    let activation = QueueActivation::try_new(epoch, QueueId::new(0), config, rings)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let status = port
        .poll_control(
            port.bus.bootstrap_queue(stage, activation),
            activation.queue(),
        )
        .await?;
    let handle = QueueConfigHandle::new(
        activation.queue().epoch(),
        QueueDirectoryEntry::new(
            activation.queue().queue_id(),
            CompletionRingId::new(activation.queue().queue_id().raw()),
            config,
        ),
    );
    port.state.with(|state| {
        if state.queue.is_some() {
            return Err(ModelError::DmaSubmissionState);
        }
        state.regions.extend(proposed);
        state.queue = Some(QueueState {
            handle,
            rings,
            status,
        });
        Ok(handle)
    })?
}

pub(super) async fn allocate_buffer<B>(port: &BusDevicePort<B>, len: usize) -> ModelResult<BufferId>
where
    B: AsyncPdmaBus,
{
    let region = port.poll_model(port.bus.allocate_dma_region(len)).await?;
    port.state.with(|state| {
        state.regions.push(region);
        region.object()
    })
}

pub(super) async fn write_buffer<B>(
    port: &BusDevicePort<B>,
    id: BufferId,
    offset: usize,
    data: &[u8],
) -> ModelResult<()>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
{
    let address = checked_range(port, id, offset, data.len())?;
    port.poll_dma(port.bus.write_dma(address, data), Some(id))
        .await
}

pub(super) async fn read_buffer<B>(
    port: &BusDevicePort<B>,
    id: BufferId,
    offset: usize,
    len: usize,
) -> ModelResult<Vec<u8>>
where
    B: AsyncPdmaBus,
    B::DmaError: BusDmaErrorAdapter,
{
    let address = checked_range(port, id, offset, len)?;
    let length = u64::try_from(len).map_err(|_| ModelError::RangeOverflow)?;
    port.poll_dma(port.bus.read_dma(address, length), Some(id))
        .await
}

fn checked_range<B>(
    port: &BusDevicePort<B>,
    id: BufferId,
    offset: usize,
    len: usize,
) -> ModelResult<DmaAddress> {
    port.state.with(|state| {
        let region = state.region(id)?;
        let end = offset.checked_add(len).ok_or(ModelError::RangeOverflow)?;
        let region_len = usize::try_from(region.length()).map_err(|_| ModelError::RangeOverflow)?;
        if end > region_len {
            return Err(ModelError::OutOfBounds {
                buffer: id,
                len: region_len,
                end,
            });
        }
        let offset = u64::try_from(offset).map_err(|_| ModelError::RangeOverflow)?;
        region
            .base()
            .checked_add(offset)
            .map_err(|_| ModelError::RangeOverflow)
    })?
}

fn ring_lengths(config: QueueConfig) -> ModelResult<[u64; 3]> {
    Ok([
        ring_length(config.command_depth(), RingRecord::Submission)?,
        ring_length(config.completion_depth(), RingRecord::Completion)?,
        ring_length(config.event_depth(), RingRecord::Event)?,
    ])
}

fn ring_length(depth: usize, record: RingRecord) -> ModelResult<u64> {
    let depth = u64::try_from(depth).map_err(|_| ModelError::DmaAllocationExhausted)?;
    RingLayout::checked_extent(depth, record.record_size())
        .map_err(|_| ModelError::DmaAllocationExhausted)
}
