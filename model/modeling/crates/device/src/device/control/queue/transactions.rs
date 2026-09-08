//! Exact queue cursor transaction preparation and commit.

use crate::device::DeviceCore;
use crate::queue::exact::{ActiveQueue, PdmaControlError, QueueStatus, exact_cursor};
use pdma_model_contract::semantic::{QueueControl, QueueCursorUpdate};

impl DeviceCore {
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
