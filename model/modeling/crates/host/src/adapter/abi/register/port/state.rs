use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::adapter::abi::{PdmaMappedRegion, PdmaRegionRequest, RegisterQueueStatus};
use crate::{PortError, PortResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiEvent, AbiRingEntry, DmaAddress, DmaRegion, QueueRingLayouts,
    RingLayout,
};
use pdma_model_contract::semantic::{
    BufferId, CompletionRingId, DeviceEpoch, QueueActivation, QueueConfig, QueueConfigHandle,
    QueueDirectoryEntry, QueueId, QueueRingObjects,
};

#[derive(Debug, Clone)]
pub(super) struct PortState {
    pub(super) epoch: DeviceEpoch,
    pub(super) queue: Option<QueueState>,
    pub(super) regions: Vec<PdmaMappedRegion>,
    pub(super) pending: Option<super::pending::PendingCommand>,
    pub(super) activation: Option<QueueConfigHandle>,
    pub(super) completion: Option<pdma_model_contract::semantic::CompletionRecord>,
    pub(super) event: Option<pdma_model_contract::semantic::EventRecord>,
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
            queue: None,
            regions: Vec::new(),
            pending: None,
            activation: None,
            completion: None,
            event: None,
        }
    }

    pub(super) fn queue(&self, handle: QueueConfigHandle) -> PortResult<QueueState> {
        let queue = self
            .queue
            .ok_or_else(|| PortError::UnknownQueue(handle.queue_id()))?;
        if queue.handle != handle {
            return Err(if queue.handle.epoch() == handle.epoch() {
                PortError::UnknownQueue(handle.queue_id())
            } else {
                PortError::StaleEpoch {
                    expected: queue.handle.epoch(),
                    actual: handle.epoch(),
                }
            });
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
    super::pending::resolve(port).await?;
    if let Some(handle) = port.with_state(|state| state.activation)? {
        if handle.config() != config {
            return Err(PortError::SubmissionState.into());
        }
        super::submission::refresh(port, handle).await?;
        port.with_state(|state| state.activation = None)?;
        return Ok(handle);
    }
    let epoch = port.with_state(|state| {
        if state.queue.is_some() {
            Err(PortError::SubmissionState)
        } else {
            Ok(state.epoch)
        }
    })??;
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
    let activation = QueueActivation::new(
        epoch,
        QueueId::new(0),
        config,
        QueueRingObjects::new(submission.object(), completion.object(), event.object()),
    );
    let handle = QueueConfigHandle::new(
        activation.queue().epoch(),
        QueueDirectoryEntry::new(
            activation.queue().queue_id(),
            CompletionRingId::new(activation.queue().queue_id().raw()),
            config,
        ),
    );
    super::bootstrap::stage_activation(port, activation, rings).await?;
    super::control::command_status(
        port,
        pdma_model_contract::abi::mmio::QueueCommand::Activate,
        rings,
        activation.queue(),
        super::pending::Effect::Activate { handle, rings },
    )
    .await?;
    port.with_state(|state| state.activation = None)?;
    Ok(handle)
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
