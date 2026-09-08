use super::DeviceCore;
use crate::queue::exact::PdmaControlError;
use hardware_interface_schema::register::RegisterSpec;
use pdma_model_contract::abi::RingCursor;
use pdma_model_contract::abi::registers::QueueRegistersRegisterId;
use pdma_model_contract::semantic::QueueId;

impl DeviceCore {
    pub(super) fn write_queue_register(
        &mut self,
        index: u32,
        register: RegisterSpec<QueueRegistersRegisterId>,
        value: u64,
    ) -> Result<bool, PdmaControlError> {
        use QueueRegistersRegisterId as Id;
        match register.id() {
            Id::Enable => self.write_queue_enable(index, register.offset(), value),
            Id::SqBase | Id::SqDepth | Id::CqBase | Id::CqDepth | Id::EqBase | Id::EqDepth => self
                .queues
                .write_configuration(index, register.offset(), value),
            Id::SqProducer | Id::CqConsumer | Id::EqConsumer | Id::Doorbell => {
                self.write_queue_cursor(index, register.id(), value)
            }
            Id::SqCaptured | Id::SqConsumer | Id::CqProducer | Id::EqProducer => {
                Err(PdmaControlError::InvalidTransaction)
            }
        }
    }

    fn write_queue_enable(
        &mut self,
        index: u32,
        offset: usize,
        value: u64,
    ) -> Result<bool, PdmaControlError> {
        if value == 0 && !self.queues.is_active(index) {
            return self
                .queues
                .read_direct(index, offset)
                .map(|_| false)
                .ok_or(PdmaControlError::InvalidQueue);
        }
        if value != 1 || self.queues.is_active(index) {
            return Err(PdmaControlError::InvalidTransaction);
        }
        self.activate_direct(index)?;
        Ok(true)
    }

    fn write_queue_cursor(
        &mut self,
        index: u32,
        register: QueueRegistersRegisterId,
        value: u64,
    ) -> Result<bool, PdmaControlError> {
        use QueueRegistersRegisterId as Id;
        let queue =
            QueueId::new(usize::try_from(index).map_err(|_| PdmaControlError::InvalidQueue)?);
        let active = self.queues.exact_ring_mut(queue)?;
        let cursor = RingCursor::new(value);
        match register {
            Id::SqProducer => {
                let previous = active.submission.ring().producer().raw();
                let changed = active.publish_submission_producer(cursor)?;
                self.trace_submission(queue, previous, value);
                Ok(changed)
            }
            Id::CqConsumer => {
                let previous = active.completion.consumer().raw();
                let changed = active.publish_completion_consumer(cursor)?;
                self.trace_consumption(queue, previous, value, true);
                Ok(changed)
            }
            Id::EqConsumer => {
                let previous = active.event.consumer().raw();
                let changed = active.publish_event_consumer(cursor)?;
                self.trace_consumption(queue, previous, value, false);
                Ok(changed)
            }
            Id::Doorbell => {
                let captured = active.capture_submission_producer(cursor)?;
                let runnable = self.runnable.runnable_len();
                self.runnable.enqueue(queue);
                let changed = captured || runnable != self.runnable.runnable_len();
                if changed {
                    self.trace.doorbell_runnable(queue);
                }
                Ok(changed)
            }
            Id::SqBase
            | Id::SqDepth
            | Id::CqBase
            | Id::CqDepth
            | Id::EqBase
            | Id::EqDepth
            | Id::Enable
            | Id::SqCaptured
            | Id::SqConsumer
            | Id::CqProducer
            | Id::EqProducer => Err(PdmaControlError::InvalidTransaction),
        }
    }
}
