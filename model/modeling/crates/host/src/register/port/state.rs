use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::register::{BusQueueStatus, PdmaMappedRegion, PdmaPreparedProposal, PdmaRegionRequest};
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{DmaAddress, RingRecord};
use pdma_model_contract::binding::{QueueActivation, QueueRingRegions, RingLayout};
use pdma_model_contract::semantic::{
    BufferId, CompletionRingId, DeviceEpoch, QueueConfig, QueueConfigHandle, QueueDirectoryEntry,
    QueueId,
};

#[derive(Debug, Clone)]
pub(super) struct PortState {
    pub(super) epoch: DeviceEpoch,
    pub(super) queue: Option<QueueState>,
    pub(super) regions: Vec<PdmaMappedRegion>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct QueueState {
    pub(super) handle: QueueConfigHandle,
    pub(super) rings: QueueRingRegions,
    pub(super) status: BusQueueStatus,
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

    pub(super) fn region(&self, id: BufferId) -> ModelResult<PdmaMappedRegion> {
        self.regions
            .iter()
            .copied()
            .find(|region| region.object() == id)
            .ok_or(ModelError::UnknownBuffer(id))
    }
}

pub(super) async fn create_queue<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    config: QueueConfig,
) -> PdmaPortResult<QueueConfigHandle> {
    let epoch = port.with_state(|state| {
        if state.queue.is_some() {
            Err(ModelError::DmaSubmissionState)
        } else {
            Ok(state.epoch)
        }
    })??;
    let lengths = ring_lengths(config)?;
    let alignment = alignment()?;
    let requests = lengths.map(|length| PdmaRegionRequest::new(length, alignment));
    let proposal = port
        .poll_provision(port.access.prepare_regions(&requests))
        .await?;
    let [submission, completion, event] = proposal.regions() else {
        return Err(ModelError::DmaSubmissionState.into());
    };
    let proposed = [*submission, *completion, *event];
    let rings = QueueRingRegions::new(
        layout(*submission, lengths[0], RingRecord::Submission)?,
        layout(*completion, lengths[1], RingRecord::Completion)?,
        layout(*event, lengths[2], RingRecord::Event)?,
    );
    let activation = QueueActivation::try_new(epoch, QueueId::new(0), config, rings)
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let bytes = port
        .poll_batch(port.access.bootstrap_queue(proposal, activation))
        .await?;
    let status = super::control::decode_status(&bytes, rings)?;
    let handle = QueueConfigHandle::new(
        activation.queue().epoch(),
        QueueDirectoryEntry::new(
            activation.queue().queue_id(),
            CompletionRingId::new(activation.queue().queue_id().raw()),
            config,
        ),
    );
    Ok(port.with_state(|state| {
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
    })??)
}

pub(super) async fn allocate_buffer<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    len: usize,
) -> PdmaPortResult<BufferId> {
    let length = u64::try_from(len).map_err(|_| ModelError::DmaAllocationExhausted)?;
    let request = PdmaRegionRequest::new(length, alignment()?);
    let region = port
        .poll_provision(port.access.allocate_region(request))
        .await?;
    port.with_state(|state| {
        state.regions.push(region);
        region.object()
    })
}

pub(super) fn write_buffer<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    id: BufferId,
    offset: usize,
    data: &[u8],
) -> PdmaPortResult<()> {
    let address = checked_range(port, id, offset, data.len())?;
    port.validate_owner()?;
    MemoryAccess::write(&port.access, address, MemoryAlignment::ONE, data)
        .map_err(crate::register::PdmaRegisterPortError::from)
}

pub(super) fn read_buffer<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    id: BufferId,
    offset: usize,
    len: usize,
) -> PdmaPortResult<Vec<u8>> {
    let address = checked_range(port, id, offset, len)?;
    port.validate_owner()?;
    let mut bytes = vec![0; len];
    MemoryAccess::read(&port.access, address, MemoryAlignment::ONE, &mut bytes)
        .map_err(crate::register::PdmaRegisterPortError::from)?;
    Ok(bytes)
}

pub(super) fn checked_range<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    id: BufferId,
    offset: usize,
    len: usize,
) -> PdmaPortResult<MemoryAddress> {
    Ok(port.with_state(|state| {
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
            .ok_or(ModelError::RangeOverflow)
    })??)
}

pub(super) const fn dma_address(address: MemoryAddress) -> DmaAddress {
    DmaAddress::new(address.raw())
}

fn alignment() -> ModelResult<MemoryAlignment> {
    MemoryAlignment::try_new(64).map_err(|_| ModelError::DmaAllocationExhausted)
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

fn layout(region: PdmaMappedRegion, length: u64, record: RingRecord) -> ModelResult<RingLayout> {
    let projected = pdma_model_contract::binding::PdmaMemoryRegion::try_new(
        region.object(),
        dma_address(region.base()),
        region.length(),
    )
    .map_err(|_| ModelError::DmaSubmissionState)?;
    RingLayout::try_new(projected, length / u64::from(record.record_size()), record)
        .map_err(|_| ModelError::DmaSubmissionState)
}
