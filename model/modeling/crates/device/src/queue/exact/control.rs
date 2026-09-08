use crate::queue::QueueBlock;
use crate::queue::exact::{ActiveQueue, PdmaControlError, QueueStatus, exact_cursor};
use pdma_model_contract::semantic::{DeviceEpoch, QueueControl, QueueCursorUpdate};

impl QueueBlock {
    pub(crate) fn control_exact_queue(
        &mut self,
        epoch: DeviceEpoch,
        request: QueueControl,
    ) -> (Result<QueueStatus, PdmaControlError>, bool) {
        match request {
            QueueControl::ReadStatus(queue) => (self.status_at_epoch(epoch, queue), false),
            QueueControl::PublishSubmissionProducer(update) => {
                self.update_exact_queue(epoch, update, ActiveQueue::publish_submission_producer)
            }
            QueueControl::CaptureSubmissionProducer(update) => {
                if update.queue().epoch() != epoch
                    || self.validate_exact(update.queue().queue_id()).is_err()
                {
                    return (Err(PdmaControlError::InvalidQueue), false);
                }
                self.update_exact_queue(epoch, update, ActiveQueue::capture_submission_producer)
            }
            QueueControl::PublishSubmissionConsumer(update) => {
                self.update_exact_queue(epoch, update, ActiveQueue::publish_submission_consumer)
            }
            QueueControl::PublishCompletionProducer(update) => {
                self.update_exact_queue(epoch, update, ActiveQueue::publish_completion_producer)
            }
            QueueControl::PublishCompletionConsumer(update) => {
                self.update_exact_queue(epoch, update, ActiveQueue::publish_completion_consumer)
            }
            QueueControl::PublishEventProducer(update) => {
                self.update_exact_queue(epoch, update, ActiveQueue::publish_event_producer)
            }
            QueueControl::PublishEventConsumer(update) => {
                self.update_exact_queue(epoch, update, ActiveQueue::publish_event_consumer)
            }
        }
    }

    fn status_at_epoch(
        &self,
        epoch: DeviceEpoch,
        queue: pdma_model_contract::semantic::EpochQueue,
    ) -> Result<QueueStatus, PdmaControlError> {
        if queue.epoch() != epoch {
            return Err(PdmaControlError::StaleEpoch {
                expected: epoch,
                actual: queue.epoch(),
            });
        }
        self.exact_ring(queue.queue_id())
            .and_then(|active| active.status())
    }

    fn update_exact_queue(
        &mut self,
        epoch: DeviceEpoch,
        update: QueueCursorUpdate,
        apply: impl FnOnce(
            &mut ActiveQueue,
            pdma_model_contract::abi::RingCursor,
        ) -> Result<bool, PdmaControlError>,
    ) -> (Result<QueueStatus, PdmaControlError>, bool) {
        if update.queue().epoch() != epoch {
            return (
                Err(PdmaControlError::StaleEpoch {
                    expected: epoch,
                    actual: update.queue().epoch(),
                }),
                false,
            );
        }
        let result = self
            .exact_ring_mut(update.queue().queue_id())
            .and_then(|queue| {
                apply(queue, exact_cursor(update.cursor()))
                    .and_then(|changed| queue.status().map(|status| (status, changed)))
            });
        match result {
            Ok((status, changed)) => (Ok(status), changed),
            Err(error) => (Err(error), false),
        }
    }
}
