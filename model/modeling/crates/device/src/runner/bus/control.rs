use super::BusControlError;
use crate::dma_space::DmaStage;
use crate::runner::state::RunnerState;
use pdma_model_contract::QueueId;
use pdma_model_contract::abi::{
    DeviceEpoch, EpochQueue, QueueActivation, RingCursor, RingCursors, RingLayout, RingRecord,
};

pub(in crate::runner) mod transactions;

#[derive(Debug, Clone, Copy)]
pub(in crate::runner) struct ActiveQueue {
    pub(in crate::runner) activation: QueueActivation,
    pub(in crate::runner) submission: RingCursors,
    pub(in crate::runner) captured_submission_producer: RingCursor,
    pub(super) completion: RingCursors,
    pub(super) event: RingCursors,
}

/// Complete typed shadow state for one active DMA queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueStatus {
    queue: EpochQueue,
    submission: RingCursors,
    captured_submission_producer: RingCursor,
    completion: RingCursors,
    event: RingCursors,
}

impl QueueStatus {
    /// Epoch-qualified queue identity.
    pub const fn queue(self) -> EpochQueue {
        self.queue
    }

    /// Submission producer/consumer shadows.
    pub const fn submission(self) -> RingCursors {
        self.submission
    }

    /// Submission producer captured by the last doorbell.
    pub const fn captured_submission_producer(self) -> RingCursor {
        self.captured_submission_producer
    }

    /// Completion producer/consumer shadows.
    pub const fn completion(self) -> RingCursors {
        self.completion
    }

    /// Event producer/consumer shadows.
    pub const fn event(self) -> RingCursors {
        self.event
    }
}

#[derive(Debug)]
pub(in crate::runner) struct BusControlState {
    epoch: DeviceEpoch,
    queues: Vec<ActiveQueue>,
}

impl BusControlState {
    pub(crate) const fn new() -> Self {
        Self {
            epoch: DeviceEpoch::initial(),
            queues: Vec::new(),
        }
    }

    pub(in crate::runner) const fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }

    pub(in crate::runner) fn reset(&mut self, epoch: DeviceEpoch) {
        self.epoch = epoch;
        self.queues.clear();
    }

    pub(in crate::runner) fn queue(
        &self,
        queue: EpochQueue,
    ) -> Result<&ActiveQueue, BusControlError> {
        self.validate_queue(queue)?;
        self.queues
            .get(queue.queue_id().raw())
            .ok_or(BusControlError::InvalidQueue)
    }

    pub(in crate::runner) fn queue_mut(
        &mut self,
        queue: EpochQueue,
    ) -> Result<&mut ActiveQueue, BusControlError> {
        self.validate_queue(queue)?;
        self.queues
            .get_mut(queue.queue_id().raw())
            .ok_or(BusControlError::InvalidQueue)
    }

    fn validate_queue(&self, queue: EpochQueue) -> Result<(), BusControlError> {
        if queue.epoch() != self.epoch {
            Err(BusControlError::StaleEpoch)
        } else if queue.queue_id().raw() >= self.queues.len() {
            Err(BusControlError::InvalidQueue)
        } else {
            Ok(())
        }
    }
}

impl ActiveQueue {
    fn new(activation: QueueActivation) -> Result<Self, BusControlError> {
        let rings = activation.rings();
        let zero = RingCursor::new(0);
        Ok(Self {
            activation,
            submission: RingCursors::try_new(rings.submission(), zero, zero)?,
            captured_submission_producer: zero,
            completion: RingCursors::try_new(rings.completion(), zero, zero)?,
            event: RingCursors::try_new(rings.event(), zero, zero)?,
        })
    }

    pub(super) const fn status(self) -> QueueStatus {
        QueueStatus {
            queue: self.activation.queue(),
            submission: self.submission,
            captured_submission_producer: self.captured_submission_producer,
            completion: self.completion,
            event: self.event,
        }
    }
}

pub(super) fn activate(
    runner: &mut RunnerState,
    proposal: DmaStage,
    activation: QueueActivation,
) -> Result<QueueStatus, BusControlError> {
    validate_activation(runner, &proposal, activation)?;
    let queue = ActiveQueue::new(activation)?;
    runner
        .bus_control
        .queues
        .try_reserve_exact(1)
        .map_err(|_| BusControlError::InvalidTransaction)?;
    let prepared = runner
        .backend
        .prepare_dma_bus_queue(activation.config())
        .map_err(|_| BusControlError::InvalidTransaction)?;
    runner.dma_space.commit(proposal)?;
    let native = runner.backend.install_dma_bus_queue(prepared);
    if native.queue_id() != activation.queue().queue_id() {
        return Err(BusControlError::InvalidTransaction);
    }
    runner.bus_control.queues.push(queue);
    Ok(queue.status())
}

fn validate_activation(
    runner: &RunnerState,
    proposal: &DmaStage,
    activation: QueueActivation,
) -> Result<(), BusControlError> {
    if activation.queue().epoch() != runner.bus_control.epoch {
        return Err(BusControlError::StaleEpoch);
    }
    if activation.queue().queue_id() != QueueId::new(runner.bus_control.queues.len()) {
        return Err(BusControlError::InvalidQueue);
    }
    if activation.queue().queue_id() != runner.backend.next_queue_id() {
        return Err(BusControlError::InvalidQueue);
    }
    if proposal.generation() != runner.dma_space.generation() || proposal.regions().len() != 3 {
        return Err(BusControlError::InvalidTransaction);
    }
    let rings = activation.rings();
    let expected = [
        rings.submission().region(),
        rings.completion().region(),
        rings.event().region(),
    ];
    if proposal.regions() != expected
        || usize::try_from(rings.event().depth()) != Ok(activation.config().completion_depth())
        || u64::try_from(activation.config().software_owner().raw()).is_err()
    {
        return Err(BusControlError::InvalidTransaction);
    }
    validate_layout(rings.submission(), RingRecord::Submission)?;
    validate_layout(rings.completion(), RingRecord::Completion)?;
    validate_layout(rings.event(), RingRecord::Event)?;
    let [submission, completion, event] = expected;
    if regions_overlap(submission, completion)
        || regions_overlap(submission, event)
        || regions_overlap(completion, event)
    {
        return Err(BusControlError::InvalidTransaction);
    }
    Ok(())
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

const fn regions_overlap(
    left: pdma_model_contract::abi::DmaRegion,
    right: pdma_model_contract::abi::DmaRegion,
) -> bool {
    let left_end = left.base().raw().saturating_add(left.length());
    let right_end = right.base().raw().saturating_add(right.length());
    left.base().raw() < right_end && right.base().raw() < left_end
}
