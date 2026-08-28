use super::{ActiveQueue, PdmaControlError, QueueStatus};
use crate::dma_space::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryStage};
use crate::runner::execution::exact_api::PreparedExactQueue;
use crate::runner::state::DeviceCore;
use pdma_model_contract::abi::{
    AbiCommand, AbiCompletion, AbiEvent, AbiRingEntry, QueueRingLayouts, RingLayout,
};
use pdma_model_contract::semantic::{QueueActivation, QueueId};

struct ValidatedQueueActivation {
    request: QueueActivation,
    layouts: QueueRingLayouts,
}

pub(in crate::runner) struct PreparedQueueActivation {
    shadow: ActiveQueue,
    native: PreparedExactQueue,
}

impl PreparedQueueActivation {
    pub(in crate::runner) const fn status(&self) -> QueueStatus {
        self.shadow.status()
    }
}

pub(in crate::runner) fn activate_staged(
    runner: &mut DeviceCore,
    stage: PdmaMemoryStage,
    activation: QueueActivation,
) -> Result<QueueStatus, PdmaControlError> {
    let prepared = prepare_staged(runner, &stage, activation)?;
    runner.dma_space.commit(stage)?;
    Ok(install(runner, prepared))
}

pub(in crate::runner) fn activate_mapped(
    runner: &mut DeviceCore,
    activation: QueueActivation,
) -> Result<QueueStatus, PdmaControlError> {
    let prepared = prepare_mapped(runner, activation)?;
    Ok(install(runner, prepared))
}

pub(in crate::runner) fn prepare_staged(
    runner: &mut DeviceCore,
    stage: &PdmaMemoryStage,
    activation: QueueActivation,
) -> Result<PreparedQueueActivation, PdmaControlError> {
    validate_identity(runner, activation)?;
    if stage.generation() != runner.dma_space.generation() {
        return Err(PdmaControlError::InvalidTransaction);
    }
    runner.dma_space.validate_stage(stage)?;
    let regions = staged_regions(stage, activation)?;
    let validated = validate_layouts(activation, regions)?;
    prepare(runner, &validated)
}

pub(in crate::runner) fn prepare_mapped(
    runner: &mut DeviceCore,
    activation: QueueActivation,
) -> Result<PreparedQueueActivation, PdmaControlError> {
    validate_identity(runner, activation)?;
    let regions = mapped_regions(runner, activation)?;
    runner.dma_space.validate_region_set(&regions)?;
    let validated = validate_layouts(activation, regions)?;
    prepare(runner, &validated)
}

fn validate_identity(
    runner: &DeviceCore,
    activation: QueueActivation,
) -> Result<(), PdmaControlError> {
    if activation.queue().epoch() != runner.bus_control.epoch {
        return Err(PdmaControlError::StaleEpoch {
            expected: runner.bus_control.epoch,
            actual: activation.queue().epoch(),
        });
    }
    let queue_id = activation.queue().queue_id();
    if queue_id != QueueId::new(runner.bus_control.queues.len())
        || queue_id != runner.backend.next_queue_id()
    {
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
            .dma_space
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
    let shadow = ActiveQueue::new(validated.request, validated.layouts)?;
    let native = runner
        .backend
        .prepare_dma_bus_queue(validated.request.config())
        .map_err(|_| PdmaControlError::InvalidTransaction)?;
    if native.queue_id() != validated.request.queue().queue_id() {
        return Err(PdmaControlError::InvalidTransaction);
    }
    Ok(PreparedQueueActivation { shadow, native })
}

pub(in crate::runner) fn install(
    runner: &mut DeviceCore,
    prepared: PreparedQueueActivation,
) -> QueueStatus {
    runner.backend.install_dma_bus_queue(prepared.native);
    runner.bus_control.queues.push(prepared.shadow);
    prepared.shadow.status()
}

const fn regions_overlap(left: MappedDmaRegion, right: MappedDmaRegion) -> bool {
    left.base().raw() < right.exclusive_end().raw()
        && right.base().raw() < left.exclusive_end().raw()
}
