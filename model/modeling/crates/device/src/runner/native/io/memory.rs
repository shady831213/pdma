use super::super::retirement::FaultRetirement;
use crate::dma_space::PdmaMemoryMappings;
use pdma_model_contract::{CommandId, DmaRangeStart, MemoryFault, QueueId};

#[derive(Debug, Clone, Copy)]
pub(in crate::runner::native) struct QueueMemoryResolver {
    queue_id: QueueId,
    command_id: CommandId,
}

impl QueueMemoryResolver {
    pub(in crate::runner::native) const fn new(queue_id: QueueId, command_id: CommandId) -> Self {
        Self {
            queue_id,
            command_id,
        }
    }

    pub(in crate::runner::native) fn validate(
        self,
        memory: &PdmaMemoryMappings,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<(), FaultRetirement> {
        memory
            .validate_range(start, len)
            .map(|_| ())
            .map_err(|fault| self.retirement(fault))
    }

    pub(in crate::runner::native) fn read(
        self,
        memory: &PdmaMemoryMappings,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<Vec<u8>, FaultRetirement> {
        memory
            .read_range(start, len)
            .map_err(|fault| self.retirement(fault))
    }

    pub(in crate::runner::native) fn write(
        self,
        memory: &PdmaMemoryMappings,
        start: DmaRangeStart,
        bytes: &[u8],
    ) -> Result<(), FaultRetirement> {
        memory
            .write_range(start, bytes)
            .map_err(|fault| self.retirement(fault))
    }

    pub(in crate::runner::native) fn strided_start(
        self,
        memory: &PdmaMemoryMappings,
        base: DmaRangeStart,
        row: usize,
        stride: usize,
    ) -> Result<DmaRangeStart, FaultRetirement> {
        let offset = row
            .checked_mul(stride)
            .and_then(|offset| base.offset().checked_add(offset))
            .ok_or_else(|| self.retirement(memory.range_overflow_fault(base)))?;
        Ok(DmaRangeStart::new(base.object(), offset))
    }

    const fn retirement(self, fault: MemoryFault) -> FaultRetirement {
        FaultRetirement {
            queue_id: self.queue_id,
            command_id: self.command_id,
            fault,
        }
    }
}
