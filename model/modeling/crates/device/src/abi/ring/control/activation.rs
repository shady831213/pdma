//! Exact-ABI queue activation preparation and commit.

use super::{PdmaControlError, QueueStatus};
use crate::device::DeviceCore;
use crate::dma::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryStage};
use crate::queue::exact::PreparedExactQueue;
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiEvent, AbiRingEntry, QueueRingLayouts, RingLayout,
};
use pdma_model_contract::semantic::QueueActivation;

struct ValidatedQueueActivation {
    request: QueueActivation,
    layouts: QueueRingLayouts,
}

pub(crate) struct PreparedQueueActivation {
    native: PreparedExactQueue,
    status: QueueStatus,
}

impl PreparedQueueActivation {
    pub(crate) const fn status(&self) -> QueueStatus {
        self.status
    }
}

pub(crate) fn activate_staged(
    runner: &mut DeviceCore,
    stage: PdmaMemoryStage,
    activation: QueueActivation,
) -> Result<QueueStatus, PdmaControlError> {
    let prepared = prepare_staged(runner, &stage, activation)?;
    runner.dma.commit(stage)?;
    Ok(install(runner, prepared))
}

pub(crate) fn activate_mapped(
    runner: &mut DeviceCore,
    activation: QueueActivation,
) -> Result<QueueStatus, PdmaControlError> {
    let prepared = prepare_mapped(runner, activation)?;
    Ok(install(runner, prepared))
}

pub(crate) fn prepare_staged(
    runner: &mut DeviceCore,
    stage: &PdmaMemoryStage,
    activation: QueueActivation,
) -> Result<PreparedQueueActivation, PdmaControlError> {
    validate_identity(runner, activation)?;
    if stage.generation() != runner.dma.generation() {
        return Err(PdmaControlError::InvalidTransaction);
    }
    runner.dma.validate_stage(stage)?;
    let regions = staged_regions(stage, activation)?;
    let validated = validate_layouts(activation, regions)?;
    prepare(runner, &validated)
}

pub(crate) fn prepare_mapped(
    runner: &mut DeviceCore,
    activation: QueueActivation,
) -> Result<PreparedQueueActivation, PdmaControlError> {
    validate_identity(runner, activation)?;
    let regions = mapped_regions(runner, activation)?;
    runner.dma.validate_region_set(&regions)?;
    let validated = validate_layouts(activation, regions)?;
    prepare(runner, &validated)
}

fn validate_identity(
    runner: &DeviceCore,
    activation: QueueActivation,
) -> Result<(), PdmaControlError> {
    if activation.queue().epoch() != runner.epoch() {
        return Err(PdmaControlError::StaleEpoch {
            expected: runner.epoch(),
            actual: activation.queue().epoch(),
        });
    }
    let queue_id = activation.queue().queue_id();
    if queue_id != runner.next_queue_id() {
        return Err(PdmaControlError::InvalidQueue);
    }
    Ok(())
}

fn staged_regions(
    stage: &PdmaMemoryStage,
    activation: QueueActivation,
) -> Result<[MappedDmaRegion; 3], PdmaControlError> {
    let [submission, completion, event] = stage.regions() else {
        return Err(PdmaControlError::InvalidTransaction);
    };
    let regions = [*submission, *completion, *event];
    let objects = activation.rings();
    if regions.map(MappedDmaRegion::object)
        != [objects.submission(), objects.completion(), objects.event()]
    {
        return Err(PdmaControlError::InvalidTransaction);
    }
    Ok(regions)
}

fn mapped_regions(
    runner: &DeviceCore,
    activation: QueueActivation,
) -> Result<[MappedDmaRegion; 3], PdmaControlError> {
    let objects = activation.rings();
    let resolve = |object| {
        runner
            .dma
            .region_for_object(object)
            .ok_or(PdmaControlError::Dma(PdmaMemoryError::InvalidRegion))
    };
    Ok([
        resolve(objects.submission())?,
        resolve(objects.completion())?,
        resolve(objects.event())?,
    ])
}

fn validate_layouts(
    activation: QueueActivation,
    regions: [MappedDmaRegion; 3],
) -> Result<ValidatedQueueActivation, PdmaControlError> {
    let config = activation.config();
    if u64::try_from(config.software_owner().raw()).is_err() {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let [submission, completion, event] = regions;
    if regions_overlap(submission, completion)
        || regions_overlap(submission, event)
        || regions_overlap(completion, event)
    {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let layouts = QueueRingLayouts::new(
        layout::<AbiCommand>(submission, config.command_depth())?,
        layout::<AbiCompletion>(completion, config.completion_depth())?,
        layout::<AbiEvent>(event, config.event_depth())?,
    );
    Ok(ValidatedQueueActivation {
        request: activation,
        layouts,
    })
}

fn layout<R: AbiRingEntry>(
    region: MappedDmaRegion,
    depth: usize,
) -> Result<RingLayout<R>, PdmaControlError> {
    let depth = u64::try_from(depth).map_err(|_| PdmaControlError::InvalidTransaction)?;
    RingLayout::try_new(region.region(), depth).map_err(PdmaControlError::from)
}

fn prepare(
    runner: &mut DeviceCore,
    validated: &ValidatedQueueActivation,
) -> Result<PreparedQueueActivation, PdmaControlError> {
    let native = runner
        .prepare_dma_bus_queue(PreparedExactQueue::new(
            validated.request,
            validated.layouts,
        ))
        .map_err(|_| PdmaControlError::InvalidTransaction)?;
    if native.queue_id() != validated.request.queue().queue_id() {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let status = native.status()?;
    Ok(PreparedQueueActivation { native, status })
}

pub(crate) fn install(runner: &mut DeviceCore, prepared: PreparedQueueActivation) -> QueueStatus {
    runner.install_dma_bus_queue(prepared.native);
    prepared.status
}

const fn regions_overlap(left: MappedDmaRegion, right: MappedDmaRegion) -> bool {
    left.base().raw() < right.exclusive_end().raw()
        && right.base().raw() < left.exclusive_end().raw()
}
