use super::{ActiveQueue, BusControlError, QueueStatus};
use crate::runner::bus::{DmaWrite, SubmissionAcceptance};
use crate::runner::state::RunnerState;
use pdma_model_contract::abi::{QueueControl, QueueCursorUpdate, RingCursor, RingRecord};

pub(in crate::runner::bus) fn accept_submission(
    state: &mut RunnerState,
    request: SubmissionAcceptance,
) -> Result<QueueStatus, BusControlError> {
    let queue = *state.bus_control.queue(request.queue)?;
    if queue.submission.producer() != request.expected_producer || queue.submission.is_full() {
        return Err(BusControlError::InvalidTransaction);
    }
    let next = request.expected_producer.checked_increment(1)?;
    let submission = queue.submission.update_producer(next)?;
    validate_descriptor(&queue, &request.descriptor)?;
    validate_indirect(state, request.indirect.as_ref())?;

    if let Some((stage, write)) = request.indirect {
        state.dma_space.commit(stage)?;
        state.dma_space.write_iova(write.address(), write.bytes())?;
    }
    state
        .dma_space
        .write_iova(request.descriptor.address(), request.descriptor.bytes())?;
    let active = state.bus_control.queue_mut(request.queue)?;
    active.submission = submission;
    Ok(active.status())
}

fn validate_descriptor(queue: &ActiveQueue, write: &DmaWrite) -> Result<(), BusControlError> {
    let layout = queue.activation.rings().submission();
    let slot_offset = layout
        .slot(queue.submission.producer())
        .checked_mul(u64::from(RingRecord::Submission.record_size()))
        .ok_or(BusControlError::InvalidTransaction)?;
    let expected = pdma_model_contract::abi::DmaAddress::new(
        layout
            .region()
            .base()
            .raw()
            .checked_add(slot_offset)
            .ok_or(BusControlError::InvalidTransaction)?,
    );
    if write.address() != expected
        || write.bytes().len() != usize::from(RingRecord::Submission.record_size())
    {
        Err(BusControlError::InvalidTransaction)
    } else {
        Ok(())
    }
}

fn validate_indirect(
    state: &RunnerState,
    indirect: Option<&(crate::dma_space::DmaStage, DmaWrite)>,
) -> Result<(), BusControlError> {
    let Some((proposal, write)) = indirect else {
        return Ok(());
    };
    let [region] = proposal.regions() else {
        return Err(BusControlError::InvalidTransaction);
    };
    if proposal.generation() != state.dma_space.generation()
        || write.address() != region.base()
        || u64::try_from(write.bytes().len()) != Ok(region.length())
    {
        Err(BusControlError::InvalidTransaction)
    } else {
        Ok(())
    }
}

pub(in crate::runner) fn control(
    state: &mut RunnerState,
    request: QueueControl,
) -> (Result<QueueStatus, BusControlError>, bool) {
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
                return (Err(BusControlError::InvalidQueue), false);
            }
            let (result, changed) = update_queue(state, update, |queue, cursor| {
                if cursor < queue.captured_submission_producer
                    || cursor > queue.submission.producer()
                {
                    return Err(BusControlError::InvalidTransaction);
                }
                let changed = cursor != queue.captured_submission_producer;
                queue.captured_submission_producer = cursor;
                Ok(changed)
            });
            if result.is_ok() && state.backend.ring_dma_bus_doorbell(update.queue()).is_err() {
                return (Err(BusControlError::InvalidQueue), false);
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
    queue_id: pdma_model_contract::QueueId,
) -> Result<(), BusControlError> {
    let queue = pdma_model_contract::abi::EpochQueue::new(control.epoch(), queue_id);
    let active = control.queue_mut(queue)?;
    let next = active.submission.consumer().checked_increment(1)?;
    if next > active.captured_submission_producer {
        return Err(BusControlError::InvalidTransaction);
    }
    active.submission = active.submission.update_consumer(next)?;
    Ok(())
}

fn update_queue(
    state: &mut RunnerState,
    update: QueueCursorUpdate,
    apply: impl FnOnce(&mut ActiveQueue, RingCursor) -> Result<bool, BusControlError>,
) -> (Result<QueueStatus, BusControlError>, bool) {
    let result = state
        .bus_control
        .queue_mut(update.queue())
        .and_then(|queue| apply(queue, update.cursor()).map(|changed| (queue.status(), changed)));
    match result {
        Ok((status, changed)) => (Ok(status), changed),
        Err(error) => (Err(error), false),
    }
}
