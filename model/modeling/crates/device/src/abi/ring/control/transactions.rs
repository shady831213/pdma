//! Exact-ABI queue cursor transaction preparation and commit.

use super::{PdmaControlError, QueueStatus};
use crate::abi::ring::{DmaWrite, SubmissionAcceptance};
use crate::device::DeviceCore;
use crate::queue::exact::{ActiveQueue, SubmissionRingState, exact_cursor};
use pdma_model_contract::semantic::{QueueControl, QueueCursorUpdate};

#[derive(Clone, Copy)]
pub(crate) struct PreparedSubmissionPublication {
    queue: pdma_model_contract::semantic::EpochQueue,
    submission: SubmissionRingState,
    status: QueueStatus,
}

impl PreparedSubmissionPublication {
    pub(crate) const fn status(&self) -> QueueStatus {
        self.status
    }
}

pub(crate) fn accept_submission(
    state: &mut DeviceCore,
    request: SubmissionAcceptance,
) -> Result<QueueStatus, PdmaControlError> {
    let queue = *state.exact_ring(request.queue)?;
    let expected_producer = exact_cursor(request.expected_producer);
    if queue.submission.ring().producer() != expected_producer || queue.submission.ring().is_full()
    {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let next = expected_producer.checked_increment(1)?;
    let submission = queue.submission.update_producer(next)?;
    validate_descriptor(&queue, &request.descriptor)?;
    validate_indirect(state, request.indirect.as_ref())?;

    if let Some((stage, write)) = request.indirect {
        state.dma.commit(stage)?;
        state.dma.write_memory(write.address(), write.bytes())?;
    }
    state
        .dma
        .write_memory(request.descriptor.address(), request.descriptor.bytes())?;
    let active = state.exact_ring_mut(request.queue)?;
    active.submission = submission;
    active.status()
}

fn validate_descriptor(queue: &ActiveQueue, write: &DmaWrite) -> Result<(), PdmaControlError> {
    let ring = queue.submission.ring();
    let expected = ring
        .entry_address(ring.producer())
        .map_err(|_| PdmaControlError::InvalidTransaction)?;
    if write.address() != expected || write.bytes().len() != usize::from(ring.layout().entry_size())
    {
        Err(PdmaControlError::InvalidTransaction)
    } else {
        Ok(())
    }
}

fn validate_indirect(
    state: &DeviceCore,
    indirect: Option<&(crate::dma::PdmaMemoryStage, DmaWrite)>,
) -> Result<(), PdmaControlError> {
    let Some((proposal, write)) = indirect else {
        return Ok(());
    };
    let [region] = proposal.regions() else {
        return Err(PdmaControlError::InvalidTransaction);
    };
    if proposal.generation() != state.dma.generation()
        || write.address() != region.base()
        || u64::try_from(write.bytes().len()) != Ok(region.length())
    {
        Err(PdmaControlError::InvalidTransaction)
    } else {
        Ok(())
    }
}

pub(crate) fn prepare_submission_publication(
    state: &DeviceCore,
    request: &SubmissionAcceptance,
) -> Result<PreparedSubmissionPublication, PdmaControlError> {
    let queue = *state.exact_ring(request.queue)?;
    let expected_producer = exact_cursor(request.expected_producer);
    if queue.submission.ring().producer() != expected_producer || queue.submission.ring().is_full()
    {
        return Err(PdmaControlError::InvalidTransaction);
    }
    let next = expected_producer.checked_increment(1)?;
    let submission = queue.submission.update_producer(next)?;
    validate_descriptor(&queue, &request.descriptor)?;
    validate_indirect(state, request.indirect.as_ref())?;
    let mut projected = queue;
    projected.submission = submission;
    Ok(PreparedSubmissionPublication {
        queue: request.queue,
        submission,
        status: projected.status()?,
    })
}

pub(crate) fn install_submission_publication(
    state: &mut DeviceCore,
    prepared: &PreparedSubmissionPublication,
) -> QueueStatus {
    if let Ok(queue) = state.exact_ring_mut(prepared.queue) {
        queue.submission = prepared.submission;
    }
    prepared.status
}

pub(crate) fn control(
    state: &mut DeviceCore,
    request: QueueControl,
) -> (Result<QueueStatus, PdmaControlError>, bool) {
    match request {
        QueueControl::ReadStatus(queue) => (
            state.exact_ring(queue).and_then(|active| active.status()),
            false,
        ),
        QueueControl::PublishSubmissionProducer(update) => {
            update_queue(state, update, |queue, cursor| {
                let updated = queue.submission.update_producer(cursor)?;
                let changed = updated != queue.submission;
                queue.submission = updated;
                Ok(changed)
            })
        }
        QueueControl::CaptureSubmissionProducer(update) => {
            if state.validate_dma_bus_queue(update.queue()).is_err() {
                return (Err(PdmaControlError::InvalidQueue), false);
            }
            let (result, changed) = update_queue(state, update, |queue, cursor| {
                let changed = cursor != queue.submission.captured_producer();
                queue.submission = queue.submission.capture(cursor)?;
                Ok(changed)
            });
            if result.is_ok() && state.ring_dma_bus_doorbell(update.queue()).is_err() {
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

pub(crate) fn advance_submission_consumer_raw(
    active: &mut ActiveQueue,
) -> Result<(), PdmaControlError> {
    let next = active.submission.ring().consumer().checked_increment(1)?;
    active.submission = active.submission.consume_captured(next)?;
    Ok(())
}

fn update_queue(
    state: &mut DeviceCore,
    update: QueueCursorUpdate,
    apply: impl FnOnce(
        &mut ActiveQueue,
        pdma_model_contract::abi::RingCursor,
    ) -> Result<bool, PdmaControlError>,
) -> (Result<QueueStatus, PdmaControlError>, bool) {
    let result = state.exact_ring_mut(update.queue()).and_then(|queue| {
        apply(queue, exact_cursor(update.cursor()))
            .and_then(|changed| queue.status().map(|status| (status, changed)))
    });
    match result {
        Ok((status, changed)) => (Ok(status), changed),
        Err(error) => (Err(error), false),
    }
}
