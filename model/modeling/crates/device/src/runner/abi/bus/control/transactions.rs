use super::{ActiveQueue, PdmaControlError, QueueStatus, SubmissionRingState};
use crate::runner::abi::bus::{DmaWrite, SubmissionAcceptance};
use crate::runner::state::DeviceCore;
use pdma_model_contract::semantic::{QueueControl, QueueCursorUpdate, RingCursor};

#[derive(Clone, Copy)]
pub(in crate::runner) struct PreparedSubmissionPublication {
    queue: pdma_model_contract::semantic::EpochQueue,
    submission: SubmissionRingState,
    status: QueueStatus,
}

impl PreparedSubmissionPublication {
    pub(in crate::runner) const fn status(&self) -> QueueStatus {
        self.status
    }
}

pub(in crate::runner::abi::bus) fn accept_submission(
    state: &mut DeviceCore,
    request: SubmissionAcceptance,
) -> Result<QueueStatus, PdmaControlError> {
    let queue = *state.bus_control.queue(request.queue)?;
    if queue.submission.ring().producer() != request.expected_producer
        || queue.submission.ring().is_full()
    {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let next = request.expected_producer.checked_increment(1)?;
    let submission = queue.submission.update_producer(next)?;
    validate_descriptor(&queue, &request.descriptor)?;
    validate_indirect(state, request.indirect.as_ref())?;

    if let Some((stage, write)) = request.indirect {
        state.dma_space.commit(stage)?;
        state
            .dma_space
            .write_memory(write.address(), write.bytes())?;
    }
    state
        .dma_space
        .write_memory(request.descriptor.address(), request.descriptor.bytes())?;
    let active = state.bus_control.queue_mut(request.queue)?;
    active.submission = submission;
    Ok(active.status())
}

fn validate_descriptor(queue: &ActiveQueue, write: &DmaWrite) -> Result<(), PdmaControlError> {
    let ring = queue.submission.ring();
    let expected = ring.entry_address(ring.producer())?;
    if write.address() != expected || write.bytes().len() != usize::from(ring.layout().entry_size())
    {
        Err(PdmaControlError::InvalidTransaction)
    } else {
        Ok(())
    }
}

fn validate_indirect(
    state: &DeviceCore,
    indirect: Option<&(crate::dma_space::PdmaMemoryStage, DmaWrite)>,
) -> Result<(), PdmaControlError> {
    let Some((proposal, write)) = indirect else {
        return Ok(());
    };
    let [region] = proposal.regions() else {
        return Err(PdmaControlError::InvalidTransaction);
    };
    if proposal.generation() != state.dma_space.generation()
        || write.address() != region.base()
        || u64::try_from(write.bytes().len()) != Ok(region.length())
    {
        Err(PdmaControlError::InvalidTransaction)
    } else {
        Ok(())
    }
}

pub(in crate::runner) fn prepare_submission_publication(
    state: &DeviceCore,
    request: &SubmissionAcceptance,
) -> Result<PreparedSubmissionPublication, PdmaControlError> {
    let queue = *state.bus_control.queue(request.queue)?;
    if queue.submission.ring().producer() != request.expected_producer
        || queue.submission.ring().is_full()
    {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let next = request.expected_producer.checked_increment(1)?;
    let submission = queue.submission.update_producer(next)?;
    validate_descriptor(&queue, &request.descriptor)?;
    validate_indirect(state, request.indirect.as_ref())?;
    let mut projected = queue;
    projected.submission = submission;
    Ok(PreparedSubmissionPublication {
        queue: request.queue,
        submission,
        status: projected.status(),
    })
}

pub(in crate::runner) fn install_submission_publication(
    state: &mut DeviceCore,
    prepared: &PreparedSubmissionPublication,
) -> QueueStatus {
    for (index, queue) in state.bus_control.queues.iter_mut().enumerate() {
        if index == prepared.queue.queue_id().raw() {
            queue.submission = prepared.submission;
            break;
        }
    }
    prepared.status
}

pub(in crate::runner) fn control(
    state: &mut DeviceCore,
    request: QueueControl,
) -> (Result<QueueStatus, PdmaControlError>, bool) {
    match request {
        QueueControl::ReadStatus(queue) => {
            (state.bus_control.queue(queue).map(|q| q.status()), false)
        }
        QueueControl::PublishSubmissionProducer(update) => {
            update_queue(state, update, |queue, cursor| {
                let updated = queue.submission.update_producer(cursor)?;
                let changed = updated != queue.submission;
                queue.submission = updated;
                Ok(changed)
            })
        }
        QueueControl::CaptureSubmissionProducer(update) => {
            if state
                .backend
                .validate_dma_bus_queue(update.queue())
                .is_err()
            {
                return (Err(PdmaControlError::InvalidQueue), false);
            }
            let (result, changed) = update_queue(state, update, |queue, cursor| {
                let changed = cursor != queue.submission.captured_producer();
                queue.submission = queue.submission.capture(cursor)?;
                Ok(changed)
            });
            if result.is_ok() && state.backend.ring_dma_bus_doorbell(update.queue()).is_err() {
                return (Err(PdmaControlError::InvalidQueue), false);
            }
            (result, changed)
        }
        QueueControl::PublishSubmissionConsumer(update) => {
            update_queue(state, update, |queue, cursor| {
                let updated = queue.submission.update_consumer(cursor)?;
                let changed = updated != queue.submission;
                queue.submission = updated;
                Ok(changed)
            })
        }
        QueueControl::PublishCompletionProducer(update) => {
            update_queue(state, update, |queue, cursor| {
                let updated = queue.completion.update_producer(cursor)?;
                let changed = updated != queue.completion;
                queue.completion = updated;
                Ok(changed)
            })
        }
        QueueControl::PublishCompletionConsumer(update) => {
            update_queue(state, update, |queue, cursor| {
                let updated = queue.completion.update_consumer(cursor)?;
                let changed = updated != queue.completion;
                queue.completion = updated;
                Ok(changed)
            })
        }
        QueueControl::PublishEventProducer(update) => {
            update_queue(state, update, |queue, cursor| {
                let updated = queue.event.update_producer(cursor)?;
                let changed = updated != queue.event;
                queue.event = updated;
                Ok(changed)
            })
        }
        QueueControl::PublishEventConsumer(update) => {
            update_queue(state, update, |queue, cursor| {
                let updated = queue.event.update_consumer(cursor)?;
                let changed = updated != queue.event;
                queue.event = updated;
                Ok(changed)
            })
        }
    }
}

pub(in crate::runner) fn advance_submission_consumer_raw(
    control: &mut super::BusControlState,
    queue_id: pdma_model_contract::semantic::QueueId,
) -> Result<(), PdmaControlError> {
    let queue = pdma_model_contract::semantic::EpochQueue::new(control.epoch(), queue_id);
    let active = control.queue_mut(queue)?;
    let next = active.submission.ring().consumer().checked_increment(1)?;
    active.submission = active.submission.consume_captured(next)?;
    Ok(())
}

fn update_queue(
    state: &mut DeviceCore,
    update: QueueCursorUpdate,
    apply: impl FnOnce(&mut ActiveQueue, RingCursor) -> Result<bool, PdmaControlError>,
) -> (Result<QueueStatus, PdmaControlError>, bool) {
    let result = state
        .bus_control
        .queue_mut(update.queue())
        .and_then(|queue| apply(queue, update.cursor()).map(|changed| (queue.status(), changed)));
    match result {
        Ok((status, changed)) => (Ok(status), changed),
        Err(error) => (Err(error), false),
    }
}
