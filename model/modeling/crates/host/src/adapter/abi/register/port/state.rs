use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::adapter::abi::{PdmaMappedRegion, PdmaRegionRequest, RegisterQueueStatus};
use crate::{PortError, PortResult};
use crate::{QueueConfig, QueueConfigHandle, QueueDirectoryEntry};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiEvent, AbiRingEntry, DmaAddress, DmaRegion, QueueRingLayouts,
    RingLayout,
};
use pdma_model_contract::semantic::{
    BufferId, CompletionRingId, DeviceEpoch, EpochQueue, QueueId, RingCursor, RingCursors,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(super) struct PortState {
    pub(super) epoch: DeviceEpoch,
    pub(super) queues: BTreeMap<QueueId, QueueState>,
    pub(super) regions: Vec<PdmaMappedRegion>,
    pub(super) completion: BTreeMap<QueueId, pdma_model_contract::semantic::CompletionRecord>,
    pub(super) event: BTreeMap<QueueId, pdma_model_contract::semantic::EventRecord>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct QueueState {
    pub(super) handle: QueueConfigHandle,
    pub(super) rings: QueueRingLayouts,
    pub(super) status: RegisterQueueStatus,
}

impl PortState {
    pub(super) const fn new(epoch: DeviceEpoch) -> Self {
        Self {
            epoch,
            queues: BTreeMap::new(),
            regions: Vec::new(),
            completion: BTreeMap::new(),
            event: BTreeMap::new(),
        }
    }

    pub(super) fn queue(&self, handle: QueueConfigHandle) -> PortResult<QueueState> {
        if handle.epoch() != self.epoch {
            return Err(PortError::StaleEpoch {
                expected: self.epoch,
                actual: handle.epoch(),
            });
        }
        let queue = self
            .queues
            .get(&handle.queue_id())
            .copied()
            .ok_or_else(|| PortError::UnknownQueue(handle.queue_id()))?;
        if queue.handle != handle {
            return Err(PortError::UnknownQueue(handle.queue_id()));
        }
        Ok(queue)
    }

    pub(super) fn region(&self, id: BufferId) -> PortResult<PdmaMappedRegion> {
        self.regions
            .iter()
            .copied()
            .find(|region| region.object() == id)
            .ok_or(PortError::UnknownBuffer(id))
    }
}

pub(super) async fn create_queue<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    config: QueueConfig,
) -> PdmaPortResult<QueueConfigHandle> {
    let _window = port.queue_window.lock().await;
    let epoch = port.with_state(|state| state.epoch)?;
    let count_offset =
        u32::try_from(pdma_model_contract::abi::registers::GlobalRegisters::QUEUE_COUNT.offset())
            .map_err(|_| PortError::SubmissionState)?;
    let index = usize::try_from(super::registers::read(port, count_offset).await?)
        .map_err(|_| PortError::SubmissionState)?;
    let queue_id = QueueId::new(index);
    let lengths = ring_lengths(config)?;
    let alignment = alignment()?;
    let mut allocated = Vec::with_capacity(3);
    for length in lengths {
        let region = port
            .poll_provision(
                port.access
                    .allocate_region(PdmaRegionRequest::new(length, alignment)),
            )
            .await?;
        port.with_state(|state| state.regions.push(region))?;
        allocated.push(region);
    }
    let [submission, completion, event] = allocated.as_slice() else {
        return Err(PortError::SubmissionState.into());
    };
    let rings = QueueRingLayouts::new(
        layout::<AbiCommand>(*submission, config.command_depth())?,
        layout::<AbiCompletion>(*completion, config.completion_depth())?,
        layout::<AbiEvent>(*event, config.event_depth())?,
    );
    let handle = QueueConfigHandle::new(
        epoch,
        QueueDirectoryEntry::new(queue_id, CompletionRingId::new(queue_id.raw()), config),
    );
    super::bootstrap::stage_activation(port, queue_id, rings).await?;
    let status = RegisterQueueStatus::new(
        EpochQueue::new(epoch, queue_id),
        empty_cursors(rings.submission().depth())?,
        RingCursor::new(0),
        empty_cursors(rings.completion().depth())?,
        empty_cursors(rings.event().depth())?,
    );
    let result = super::registers::write(
        port,
        super::registers::queue_offset(
            queue_id,
            pdma_model_contract::abi::registers::QueueRegisters::ENABLE.offset(),
        )?,
        1,
    )
    .await;
    if super::registers::committed(&result) {
        port.with_state(|state| {
            state.queues.insert(
                queue_id,
                QueueState {
                    handle,
                    rings,
                    status,
                },
            )
        })?;
    }
    result?;
    Ok(handle)
}

fn empty_cursors(depth: u32) -> PortResult<RingCursors> {
    RingCursors::try_new(depth, RingCursor::new(0), RingCursor::new(0))
        .map_err(|_| PortError::SubmissionState)
}

pub(super) async fn allocate_buffer<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    len: usize,
) -> PdmaPortResult<BufferId> {
    let length = u64::try_from(len).map_err(|_| PortError::AllocationExhausted)?;
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
        .map_err(crate::adapter::abi::PdmaRegisterPortError::from)
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
        .map_err(crate::adapter::abi::PdmaRegisterPortError::from)?;
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
        let end = offset.checked_add(len).ok_or(PortError::RangeOverflow)?;
        let region_len = usize::try_from(region.length()).map_err(|_| PortError::RangeOverflow)?;
        if end > region_len {
            return Err(PortError::OutOfBounds {
                buffer: id,
                len: region_len,
                end,
            });
        }
        let offset = u64::try_from(offset).map_err(|_| PortError::RangeOverflow)?;
        region
            .base()
            .checked_add(offset)
            .ok_or(PortError::RangeOverflow)
    })??)
}

pub(super) const fn dma_address(address: MemoryAddress) -> DmaAddress {
    DmaAddress::new(address.raw())
}

fn alignment() -> PortResult<MemoryAlignment> {
    MemoryAlignment::try_new(64).map_err(|_| PortError::AllocationExhausted)
}

fn ring_lengths(config: QueueConfig) -> PortResult<[u64; 3]> {
    Ok([
        ring_length::<AbiCommand>(config.command_depth())?,
        ring_length::<AbiCompletion>(config.completion_depth())?,
        ring_length::<AbiEvent>(config.event_depth())?,
    ])
}

fn ring_length<R: AbiRingEntry>(depth: usize) -> PortResult<u64> {
    let depth = u64::try_from(depth).map_err(|_| PortError::AllocationExhausted)?;
    RingLayout::<R>::checked_extent(depth).map_err(|_| PortError::AllocationExhausted)
}

fn layout<R: AbiRingEntry>(region: PdmaMappedRegion, depth: usize) -> PortResult<RingLayout<R>> {
    let projected = DmaRegion::try_new(dma_address(region.base()), region.length())
        .map_err(|_| PortError::SubmissionState)?;
    let depth = u64::try_from(depth).map_err(|_| PortError::SubmissionState)?;
    RingLayout::try_new(projected, depth).map_err(|_| PortError::SubmissionState)
}
