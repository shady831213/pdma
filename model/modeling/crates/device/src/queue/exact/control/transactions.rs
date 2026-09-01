//! Exact queue cursor transaction preparation and commit.

use super::super::{
    ActiveQueue, DmaWrite, PdmaControlError, QueueStatus, SubmissionAcceptance,
    SubmissionRingState, exact_cursor,
};
use crate::device::DeviceCore;
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

impl DeviceCore {
    pub(crate) fn accept_exact_submission(
        &mut self,
        request: SubmissionAcceptance,
    ) -> Result<QueueStatus, PdmaControlError> {
        let queue = *self.exact_ring(request.queue)?;
        let submission = queue
            .prepare_submission(exact_cursor(request.expected_producer), &request.descriptor)?;
        validate_indirect(self, request.indirect.as_ref())?;

        if let Some((stage, write)) = request.indirect {
            self.dma.commit(stage)?;
            self.dma.write_memory(write.address(), write.bytes())?;
        }
        self.dma
            .write_memory(request.descriptor.address(), request.descriptor.bytes())?;
        let active = self.exact_ring_mut(request.queue)?;
        active.submission = submission;
        active.status()
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

impl DeviceCore {
    pub(crate) fn prepare_submission_publication(
        &self,
        request: &SubmissionAcceptance,
    ) -> Result<PreparedSubmissionPublication, PdmaControlError> {
        let queue = *self.exact_ring(request.queue)?;
        let submission = queue
            .prepare_submission(exact_cursor(request.expected_producer), &request.descriptor)?;
        validate_indirect(self, request.indirect.as_ref())?;
        let mut projected = queue;
        projected.submission = submission;
        Ok(PreparedSubmissionPublication {
            queue: request.queue,
            submission,
            status: projected.status()?,
        })
    }

    pub(crate) fn install_submission_publication(
        &mut self,
        prepared: &PreparedSubmissionPublication,
    ) -> QueueStatus {
        if let Ok(queue) = self.exact_ring_mut(prepared.queue) {
            queue.submission = prepared.submission;
        }
        prepared.status
    }

    pub(crate) fn control_exact_queue(
        &mut self,
        request: QueueControl,
    ) -> (Result<QueueStatus, PdmaControlError>, bool) {
        match request {
            QueueControl::ReadStatus(queue) => (
                self.exact_ring(queue).and_then(|active| active.status()),
                false,
            ),
            QueueControl::PublishSubmissionProducer(update) => {
                self.update_exact_queue(update, ActiveQueue::publish_submission_producer)
            }
            QueueControl::CaptureSubmissionProducer(update) => {
                if self.validate_dma_bus_queue(update.queue()).is_err() {
                    return (Err(PdmaControlError::InvalidQueue), false);
                }
                let (result, changed) =
                    self.update_exact_queue(update, ActiveQueue::capture_submission_producer);
                if result.is_ok() && self.ring_dma_bus_doorbell(update.queue()).is_err() {
                    return (Err(PdmaControlError::InvalidQueue), false);
                }
                (result, changed)
            }
            QueueControl::PublishSubmissionConsumer(update) => {
                self.update_exact_queue(update, ActiveQueue::publish_submission_consumer)
            }
            QueueControl::PublishCompletionProducer(update) => {
                self.update_exact_queue(update, ActiveQueue::publish_completion_producer)
            }
            QueueControl::PublishCompletionConsumer(update) => {
                self.update_exact_queue(update, ActiveQueue::publish_completion_consumer)
            }
            QueueControl::PublishEventProducer(update) => {
                self.update_exact_queue(update, ActiveQueue::publish_event_producer)
            }
            QueueControl::PublishEventConsumer(update) => {
                self.update_exact_queue(update, ActiveQueue::publish_event_consumer)
            }
        }
    }

    fn update_exact_queue(
        &mut self,
        update: QueueCursorUpdate,
        apply: impl FnOnce(
            &mut ActiveQueue,
            pdma_model_contract::abi::RingCursor,
        ) -> Result<bool, PdmaControlError>,
    ) -> (Result<QueueStatus, PdmaControlError>, bool) {
        let result = self.exact_ring_mut(update.queue()).and_then(|queue| {
            apply(queue, exact_cursor(update.cursor()))
                .and_then(|changed| queue.status().map(|status| (status, changed)))
        });
        match result {
            Ok((status, changed)) => (Ok(status), changed),
            Err(error) => (Err(error), false),
        }
    }
}
