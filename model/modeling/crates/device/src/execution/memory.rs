use crate::dma::PdmaMemoryMappings;
use crate::execution::ExecutionFault;
use pdma_model_contract::semantic::{CommandId, DmaRangeStart, MemoryFault, QueueId};

#[derive(Debug, Clone, Copy)]
pub(super) struct QueueMemoryResolver {
    queue_id: QueueId,
    command_id: CommandId,
}

impl QueueMemoryResolver {
    pub(super) const fn new(queue_id: QueueId, command_id: CommandId) -> Self {
        Self {
            queue_id,
            command_id,
        }
    }

    pub(super) fn validate(
        self,
        memory: &PdmaMemoryMappings,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<(), ExecutionFault> {
        memory
            .validate_range(start, len)
            .map(|_| ())
            .map_err(|fault| self.retirement(fault))
    }

    pub(super) fn read(
        self,
        memory: &PdmaMemoryMappings,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<Vec<u8>, ExecutionFault> {
        memory
            .read_range(start, len)
            .map_err(|fault| self.retirement(fault))
    }

    pub(super) fn write(
        self,
        memory: &PdmaMemoryMappings,
        start: DmaRangeStart,
        bytes: &[u8],
    ) -> Result<(), ExecutionFault> {
        memory
            .write_range(start, bytes)
            .map_err(|fault| self.retirement(fault))
    }

    pub(super) fn strided_start(
        self,
        memory: &PdmaMemoryMappings,
        base: DmaRangeStart,
        row: usize,
        stride: usize,
    ) -> Result<DmaRangeStart, ExecutionFault> {
        let offset = row
            .checked_mul(stride)
            .and_then(|offset| base.offset().checked_add(offset))
            .ok_or_else(|| self.retirement(memory.range_overflow_fault(base)))?;
        Ok(DmaRangeStart::new(base.object(), offset))
    }

    const fn retirement(self, fault: MemoryFault) -> ExecutionFault {
        ExecutionFault {
            queue: self.queue_id,
            command: self.command_id,
            fault,
        }
    }
}
