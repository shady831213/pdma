use crate::device::flow::ExecutionFault;
use crate::memory::PdmaMemoryMappings;
use pdma_model_contract::abi::{
    AbiBounds, AbiEventAccess, AbiEventStage, AbiFault, AbiFaultSite, AbiStrideOffset, DmaAddress,
};
use pdma_model_contract::semantic::{CommandId, EventErrorKind, QueueId};

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
        address: DmaAddress,
        length: u64,
        site: AbiFaultSite,
    ) -> Result<(), ExecutionFault> {
        let end = address.checked_add(length).map_err(|_| {
            self.fault(
                memory,
                address,
                EventErrorKind::RangeOverflow,
                AbiFault::range_overflow_direct(address, length, site),
            )
        })?;
        let Some(region) = memory.region_containing(address) else {
            return Err(self.unmapped(memory, address, length, site));
        };
        if end > region.exclusive_end() {
            let error_kind = EventErrorKind::OutOfBounds {
                len: usize::try_from(region.length()).unwrap_or(usize::MAX),
                end: usize::try_from(end.raw() - region.base().raw()).unwrap_or(usize::MAX),
            };
            return Err(self.fault(
                memory,
                address,
                error_kind,
                AbiFault::out_of_bounds(
                    region.exclusive_end(),
                    AbiBounds::new(region.length(), end.raw()),
                    site,
                ),
            ));
        }
        Ok(())
    }

    pub(super) fn read(
        self,
        memory: &PdmaMemoryMappings,
        address: DmaAddress,
        length: u64,
        site: AbiFaultSite,
    ) -> Result<Vec<u8>, ExecutionFault> {
        self.validate(memory, address, length, site)?;
        memory
            .read_memory(address, length)
            .map_err(|_| self.unmapped(memory, address, length, site))
    }

    pub(super) fn write(
        self,
        memory: &PdmaMemoryMappings,
        address: DmaAddress,
        bytes: &[u8],
        site: AbiFaultSite,
    ) -> Result<(), ExecutionFault> {
        let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        self.validate(memory, address, length, site)?;
        memory
            .write_memory(address, bytes)
            .map_err(|_| self.unmapped(memory, address, length, site))
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
            .ok_or_else(|| ExecutionFault {
                queue: self.queue_id,
                command: self.command_id,
                buffer: None,
                error_kind: EventErrorKind::RangeOverflow,
                exact: AbiFault::range_overflow_strided(
                    base,
                    AbiStrideOffset::new(u64::from(row), u64::from(stride)),
                    site,
                ),
            })
    }

    pub(super) fn indirect_list(
        self,
        memory: &PdmaMemoryMappings,
        address: DmaAddress,
        count: u32,
    ) -> Result<Vec<u8>, ExecutionFault> {
        let length = u64::from(count) * 8;
        address.checked_add(length).map_err(|_| {
            self.fault(
                memory,
                address,
                EventErrorKind::RangeOverflow,
                AbiFault::range_overflow_indirect(address, u64::from(count)),
            )
        })?;
        self.read(
            memory,
            address,
            length,
            AbiFaultSite::new(AbiEventStage::IndirectList, AbiEventAccess::Read, 0),
        )
    }

    fn unmapped(
        self,
        memory: &PdmaMemoryMappings,
        address: DmaAddress,
        length: u64,
        site: AbiFaultSite,
    ) -> ExecutionFault {
        self.fault(
            memory,
            address,
            EventErrorKind::UnknownBuffer,
            AbiFault::unmapped(address, length, site),
        )
    }

    fn fault(
        self,
        memory: &PdmaMemoryMappings,
        address: DmaAddress,
        error_kind: EventErrorKind,
        exact: AbiFault,
    ) -> ExecutionFault {
        let buffer = memory
            .region_containing(address)
            .map(crate::memory::MappedDmaRegion::object);
        ExecutionFault {
            queue: self.queue_id,
            command: self.command_id,
            buffer,
            error_kind,
            exact,
        }
    }
}
