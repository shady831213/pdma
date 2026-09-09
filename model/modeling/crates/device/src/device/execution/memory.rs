use crate::device::flow::ExecutionFault;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use pdma_model_contract::abi::{
    AbiEventAccess, AbiEventStage, AbiFault, AbiFaultSite, AbiStrideOffset, DmaAddress,
};
use pdma_model_contract::semantic::{CommandId, QueueId};

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
        memory: &SharedMemoryAccess,
        address: DmaAddress,
        length: u64,
        site: AbiFaultSite,
    ) -> Result<(), ExecutionFault> {
        memory
            .validate(
                MemoryAddress::new(address.raw()),
                MemoryAlignment::ONE,
                length,
            )
            .map_err(|error| self.fault(crate::memory::fault(error, length, site)))
    }

    pub(super) fn read(
        self,
        memory: &SharedMemoryAccess,
        address: DmaAddress,
        length: u64,
        site: AbiFaultSite,
    ) -> Result<Vec<u8>, ExecutionFault> {
        crate::memory::read(memory, address, length)
            .map_err(|error| self.fault(crate::memory::fault(error, length, site)))
    }

    pub(super) fn write(
        self,
        memory: &SharedMemoryAccess,
        address: DmaAddress,
        bytes: &[u8],
        site: AbiFaultSite,
    ) -> Result<(), ExecutionFault> {
        let length = u64::try_from(bytes.len())
            .map_err(|_| self.fault(AbiFault::range_overflow_direct(address, u64::MAX, site)))?;
        self.validate(memory, address, length, site)?;
        crate::memory::write(memory, address, bytes)
            .map_err(|error| self.fault(crate::memory::fault(error, length, site)))
    }

    pub(super) fn strided_address(
        self,
        base: DmaAddress,
        row: u32,
        stride: u32,
        site: AbiFaultSite,
    ) -> Result<DmaAddress, ExecutionFault> {
        u64::from(row)
            .checked_mul(u64::from(stride))
            .and_then(|offset| base.raw().checked_add(offset))
            .map(DmaAddress::new)
            .ok_or_else(|| {
                self.fault(AbiFault::range_overflow_strided(
                    base,
                    AbiStrideOffset::new(u64::from(row), u64::from(stride)),
                    site,
                ))
            })
    }

    pub(super) fn indirect_list(
        self,
        memory: &SharedMemoryAccess,
        address: DmaAddress,
        count: u32,
    ) -> Result<Vec<u8>, ExecutionFault> {
        let length = u64::from(count) * 8;
        address.checked_add(length).map_err(|_| {
            self.fault(AbiFault::range_overflow_indirect(address, u64::from(count)))
        })?;
        self.read(
            memory,
            address,
            length,
            AbiFaultSite::new(AbiEventStage::IndirectList, AbiEventAccess::Read, 0),
        )
    }

    const fn fault(self, exact: AbiFault) -> ExecutionFault {
        ExecutionFault {
            queue: self.queue_id,
            command: self.command_id,
            exact,
        }
    }
}
