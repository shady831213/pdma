use super::DeviceCore;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadValues, AbiCompletion, AbiEvent, AbiRingEntry, RingCursor,
};
use pdma_model_contract::semantic::{CommandId, QueueId, TraceOperation};

impl DeviceCore {
    pub(super) fn trace_submission(&mut self, queue: QueueId, from: u64, to: u64) {
        if !self.trace.is_enabled() {
            return;
        }
        let Ok(active) = self.queues.exact_ring(queue).copied() else {
            return;
        };
        for cursor in from..to {
            let Ok(address) = active
                .submission
                .ring()
                .layout()
                .entry_address(RingCursor::new(cursor))
            else {
                continue;
            };
            let Ok(bytes) = self
                .memory
                .read_memory(address, u64::from(AbiCommand::ENTRY_SIZE))
            else {
                continue;
            };
            let Ok(command) = AbiCommand::decode(&bytes) else {
                continue;
            };
            let Ok(id) = usize::try_from(command.command_id()) else {
                continue;
            };
            let operation = match command.payload {
                AbiCommandPayloadValues::Copy(_) => TraceOperation::Copy,
                AbiCommandPayloadValues::StridedCopy(_) => TraceOperation::StridedCopy,
                AbiCommandPayloadValues::BroadcastCopy(_) => TraceOperation::BroadcastCopy,
                AbiCommandPayloadValues::VectorAdd(_) => TraceOperation::VectorAdd,
            };
            self.trace
                .command_submitted(queue, operation, CommandId::new(id));
        }
    }

    pub(super) fn trace_consumption(
        &mut self,
        queue: QueueId,
        from: u64,
        to: u64,
        completion: bool,
    ) {
        if !self.trace.is_enabled() {
            return;
        }
        let Ok(active) = self.queues.exact_ring(queue).copied() else {
            return;
        };
        for cursor in from..to {
            let cursor = RingCursor::new(cursor);
            if completion {
                let Ok(address) = active.completion.layout().entry_address(cursor) else {
                    continue;
                };
                let Ok(bytes) = self
                    .memory
                    .read_memory(address, u64::from(AbiCompletion::ENTRY_SIZE))
                else {
                    continue;
                };
                let Ok(record) = AbiCompletion::decode(&bytes) else {
                    continue;
                };
                self.trace.completion_polled(record);
            } else {
                let Ok(address) = active.event.layout().entry_address(cursor) else {
                    continue;
                };
                let Ok(bytes) = self
                    .memory
                    .read_memory(address, u64::from(AbiEvent::ENTRY_SIZE))
                else {
                    continue;
                };
                let Ok(record) = AbiEvent::decode(&bytes) else {
                    continue;
                };
                let Ok(command) = usize::try_from(record.command_id) else {
                    continue;
                };
                self.trace.dma_event_polled(queue, CommandId::new(command));
            }
        }
    }
}
