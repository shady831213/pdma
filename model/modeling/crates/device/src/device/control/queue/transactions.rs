use crate::device::DeviceCore;
use crate::queue::exact::{PdmaControlError, QueueStatus};
use pdma_model_contract::semantic::QueueControl;

impl DeviceCore {
    pub(crate) fn control_exact_queue(
        &mut self,
        request: QueueControl,
    ) -> (Result<QueueStatus, PdmaControlError>, bool) {
        let (result, changed) = self.queues.control_exact_queue(self.epoch, request);
        match request {
            QueueControl::CaptureSubmissionProducer(update) if result.is_ok() => {
                self.runnable.enqueue(update.queue().queue_id());
            }
            QueueControl::ReadStatus(_)
            | QueueControl::PublishSubmissionProducer(_)
            | QueueControl::CaptureSubmissionProducer(_)
            | QueueControl::PublishSubmissionConsumer(_)
            | QueueControl::PublishCompletionProducer(_)
            | QueueControl::PublishCompletionConsumer(_)
            | QueueControl::PublishEventProducer(_)
            | QueueControl::PublishEventConsumer(_) => {}
        }
        (result, changed)
    }
}
