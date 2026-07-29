use super::{QueuePortDeviceModel, retirement::FaultRetirement};
use crate::dma_space::DmaSpace;
use pdma_model_contract::{CommandId, DmaRangeStart, MemoryFault, QueueId};

pub(in crate::runner) struct NativeExecution<'a> {
    pub(in crate::runner) device: &'a mut QueuePortDeviceModel,
    pub(in crate::runner) dma_space: &'a mut DmaSpace,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SemanticMemory {
    queue_id: QueueId,
    command_id: CommandId,
}

impl SemanticMemory {
    pub(super) const fn new(queue_id: QueueId, command_id: CommandId) -> Self {
        Self {
            queue_id,
            command_id,
        }
    }

    pub(super) fn validate(
        self,
        dma_space: &DmaSpace,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<(), FaultRetirement> {
        dma_space
            .validate_range(start, len)
            .map(|_| ())
            .map_err(|fault| self.retirement(fault))
    }

    pub(super) fn read(
        self,
        dma_space: &DmaSpace,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<Vec<u8>, FaultRetirement> {
        dma_space
            .read_range(start, len)
            .map_err(|fault| self.retirement(fault))
    }

    pub(super) fn write(
        self,
        dma_space: &mut DmaSpace,
        start: DmaRangeStart,
        bytes: &[u8],
    ) -> Result<(), FaultRetirement> {
        dma_space
            .write_range(start, bytes)
            .map_err(|fault| self.retirement(fault))
    }

    pub(super) fn strided_start(
        self,
        dma_space: &DmaSpace,
        base: DmaRangeStart,
        row: usize,
        stride: usize,
    ) -> Result<DmaRangeStart, FaultRetirement> {
        let offset = row
            .checked_mul(stride)
            .and_then(|offset| base.offset().checked_add(offset))
            .ok_or_else(|| self.retirement(dma_space.range_overflow_fault(base)))?;
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
