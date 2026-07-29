use super::super::retirement::FaultRetirement;
use crate::dma_space::DmaSpace;
use pdma_model_contract::{CommandId, DmaRangeStart, MemoryFault, QueueId};

#[derive(Debug, Clone, Copy)]
enum MemoryFrontend {
    Semantic,
    DmaBus,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runner::native) struct QueueMemoryResolver {
    frontend: MemoryFrontend,
    queue_id: QueueId,
    command_id: CommandId,
}

impl QueueMemoryResolver {
    pub(super) const fn semantic(queue_id: QueueId, command_id: CommandId) -> Self {
        Self {
            frontend: MemoryFrontend::Semantic,
            queue_id,
            command_id,
        }
    }

    pub(super) const fn dma_bus(queue_id: QueueId, command_id: CommandId) -> Self {
        Self {
            frontend: MemoryFrontend::DmaBus,
            queue_id,
            command_id,
        }
    }

    pub(in crate::runner::native) fn validate(
        self,
        dma_space: &DmaSpace,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<(), FaultRetirement> {
        self.resolve(dma_space, start, len).map(|_| ())
    }

    pub(in crate::runner::native) fn read(
        self,
        dma_space: &DmaSpace,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<Vec<u8>, FaultRetirement> {
        match self.frontend {
            MemoryFrontend::Semantic | MemoryFrontend::DmaBus => dma_space
                .read_range(start, len)
                .map_err(|fault| self.retirement(fault)),
        }
    }

    pub(in crate::runner::native) fn write(
        self,
        dma_space: &mut DmaSpace,
        start: DmaRangeStart,
        bytes: &[u8],
    ) -> Result<(), FaultRetirement> {
        match self.frontend {
            MemoryFrontend::Semantic | MemoryFrontend::DmaBus => dma_space
                .write_range(start, bytes)
                .map_err(|fault| self.retirement(fault)),
        }
    }

    pub(in crate::runner::native) fn strided_start(
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

    fn resolve(
        self,
        dma_space: &DmaSpace,
        start: DmaRangeStart,
        len: usize,
    ) -> Result<DmaRangeStart, FaultRetirement> {
        match self.frontend {
            MemoryFrontend::Semantic | MemoryFrontend::DmaBus => dma_space
                .validate_range(start, len)
                .map(|_| start)
                .map_err(|fault| self.retirement(fault)),
        }
    }

    const fn retirement(self, fault: MemoryFault) -> FaultRetirement {
        FaultRetirement {
            queue_id: self.queue_id,
            command_id: self.command_id,
            fault,
        }
    }
}
