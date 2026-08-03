use super::{ActiveQueue, BusControlError, QueueStatus};
use crate::dma_space::DmaStage;
use crate::runner::native::dma_bus_api::PreparedDmaBusQueue;
use crate::runner::state::RunnerState;
use pdma_model_contract::QueueId;
use pdma_model_contract::abi::{DmaRegion, QueueActivation, RingLayout, RingRecord};

struct ValidatedMappedActivation(QueueActivation);

pub(in crate::runner) struct PreparedQueueActivation {
    shadow: ActiveQueue,
    native: PreparedDmaBusQueue,
}

pub(in crate::runner) fn activate_staged(
    runner: &mut RunnerState,
    stage: DmaStage,
    activation: QueueActivation,
) -> Result<QueueStatus, BusControlError> {
    let prepared = prepare_staged(runner, &stage, activation)?;
    runner.dma_space.commit(stage)?;
    Ok(install(runner, prepared))
}

pub(in crate::runner) fn activate_mapped(
    runner: &mut RunnerState,
    activation: QueueActivation,
) -> Result<QueueStatus, BusControlError> {
    let prepared = prepare_mapped(runner, activation)?;
    Ok(install(runner, prepared))
}

pub(in crate::runner) fn prepare_staged(
    runner: &mut RunnerState,
    stage: &DmaStage,
    activation: QueueActivation,
) -> Result<PreparedQueueActivation, BusControlError> {
    let validated = validate_staged(runner, stage, activation)?;
    prepare(runner, &validated)
}

pub(in crate::runner) fn prepare_mapped(
    runner: &mut RunnerState,
    activation: QueueActivation,
) -> Result<PreparedQueueActivation, BusControlError> {
    let validated = validate_mapped(runner, activation)?;
    prepare(runner, &validated)
}

fn validate_staged(
    runner: &RunnerState,
    stage: &DmaStage,
    activation: QueueActivation,
) -> Result<ValidatedMappedActivation, BusControlError> {
    validate_identity(runner, activation)?;
    if stage.generation() != runner.dma_space.generation() || stage.regions().len() != 3 {
        return Err(BusControlError::InvalidTransaction);
    }
    runner.dma_space.validate_stage(stage)?;
    if stage.regions() != activation_regions(activation) {
        return Err(BusControlError::InvalidTransaction);
    }
    validate_layouts(activation)?;
    Ok(ValidatedMappedActivation(activation))
}

fn validate_mapped(
    runner: &RunnerState,
    activation: QueueActivation,
) -> Result<ValidatedMappedActivation, BusControlError> {
    validate_identity(runner, activation)?;
    validate_layouts(activation)?;
    runner
        .dma_space
        .validate_region_set(&activation_regions(activation))?;
    Ok(ValidatedMappedActivation(activation))
}

fn validate_identity(
    runner: &RunnerState,
    activation: QueueActivation,
) -> Result<(), BusControlError> {
    if activation.queue().epoch() != runner.bus_control.epoch {
        return Err(BusControlError::StaleEpoch {
            expected: runner.bus_control.epoch,
            actual: activation.queue().epoch(),
        });
    }
    let queue_id = activation.queue().queue_id();
    if queue_id != QueueId::new(runner.bus_control.queues.len())
        || queue_id != runner.backend.next_queue_id()
    {
        return Err(BusControlError::InvalidQueue);
    }
    Ok(())
}

fn validate_layouts(activation: QueueActivation) -> Result<(), BusControlError> {
    let rings = activation.rings();
    if usize::try_from(rings.event().depth()) != Ok(activation.config().completion_depth())
        || u64::try_from(activation.config().software_owner().raw()).is_err()
    {
        return Err(BusControlError::InvalidTransaction);
    }
    validate_layout(rings.submission(), RingRecord::Submission)?;
    validate_layout(rings.completion(), RingRecord::Completion)?;
    validate_layout(rings.event(), RingRecord::Event)?;
    let [submission, completion, event] = activation_regions(activation);
    if regions_overlap(submission, completion)
        || regions_overlap(submission, event)
        || regions_overlap(completion, event)
    {
        return Err(BusControlError::InvalidTransaction);
    }
    Ok(())
}

fn prepare(
    runner: &mut RunnerState,
    validated: &ValidatedMappedActivation,
) -> Result<PreparedQueueActivation, BusControlError> {
    let activation = validated.0;
    let shadow = ActiveQueue::new(activation)?;
    runner
        .bus_control
        .queues
        .try_reserve_exact(1)
        .map_err(|_| BusControlError::InvalidTransaction)?;
    let native = runner
        .backend
        .prepare_dma_bus_queue(activation.config())
        .map_err(|_| BusControlError::InvalidTransaction)?;
    if native.queue_id() != activation.queue().queue_id() {
        return Err(BusControlError::InvalidTransaction);
    }
    Ok(PreparedQueueActivation { shadow, native })
}

pub(in crate::runner) fn install(
    runner: &mut RunnerState,
    prepared: PreparedQueueActivation,
) -> QueueStatus {
    runner.backend.install_dma_bus_queue(prepared.native);
    runner.bus_control.queues.push(prepared.shadow);
    prepared.shadow.status()
}

fn validate_layout(layout: RingLayout, record: RingRecord) -> Result<(), BusControlError> {
    if layout.record() != record
        || layout.extent() != layout.region().length()
        || layout.region().base().raw() % record.alignment() != 0
    {
        Err(BusControlError::InvalidTransaction)
    } else {
        Ok(())
    }
}

const fn activation_regions(activation: QueueActivation) -> [DmaRegion; 3] {
    let rings = activation.rings();
    [
        rings.submission().region(),
        rings.completion().region(),
        rings.event().region(),
    ]
}

const fn regions_overlap(left: DmaRegion, right: DmaRegion) -> bool {
    let left_end = left.base().raw().saturating_add(left.length());
    let right_end = right.base().raw().saturating_add(right.length());
    left.base().raw() < right_end && right.base().raw() < left_end
}
