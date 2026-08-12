use super::{DmaSpace, DmaSpaceError};
use crate::{ModelError, ModelResult};
use pdma_model_contract::dma::DmaRangeStart;
use pdma_model_contract::event::{EventErrorKind, MemoryFault};
use pdma_model_contract::ids::BufferId;
use pdma_model_contract::{DmaAddress, DmaRegion, bridge::fault_address};

impl DmaSpace {
    /// Resolves a semantic DMA object to its committed region.
    pub fn region_for_object(&self, object: BufferId) -> Option<DmaRegion> {
        self.regions.get(object.raw()).map(|backing| backing.region)
    }

    /// Resolves an IOVA to the one committed region containing that byte.
    pub fn region_containing(&self, address: DmaAddress) -> Option<DmaRegion> {
        self.regions
            .iter()
            .find(|backing| {
                backing.region.base() <= address && address < backing.region.exclusive_end()
            })
            .map(|backing| backing.region)
    }

    /// Reads a checked range through a semantic object and platform-sized offset.
    pub(crate) fn read_object(
        &self,
        object: BufferId,
        offset: usize,
        length: usize,
    ) -> ModelResult<Vec<u8>> {
        self.read_range(DmaRangeStart::new(object, offset), length)
            .map_err(|fault| ModelError::from_memory_fault(fault.buffer(), fault.error_kind()))
    }

    pub(crate) fn read_range(
        &self,
        start: DmaRangeStart,
        length: usize,
    ) -> Result<Vec<u8>, MemoryFault> {
        let end = self.validate_range(start, length)?;
        let backing = self
            .regions
            .get(start.object().raw())
            .ok_or_else(|| Self::unknown_fault(start.object()))?;
        let range = backing
            .bytes
            .get(start.offset()..end)
            .ok_or_else(|| self.out_of_bounds_fault(start.object(), backing.bytes.len(), end))?;
        Ok(range.to_vec())
    }

    /// Writes a checked range through a semantic object and platform-sized offset.
    pub(crate) fn write_object(
        &mut self,
        object: BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        self.write_range(DmaRangeStart::new(object, offset), data)
            .map_err(|fault| ModelError::from_memory_fault(fault.buffer(), fault.error_kind()))
    }

    pub(crate) fn write_range(
        &mut self,
        start: DmaRangeStart,
        data: &[u8],
    ) -> Result<(), MemoryFault> {
        let end = self.validate_range(start, data.len())?;
        let object = start.object();
        let backing = self
            .regions
            .get_mut(object.raw())
            .ok_or_else(|| MemoryFault::new(object, EventErrorKind::UnknownBuffer))?;
        let buffer_len = backing.bytes.len();
        let range = backing.bytes.get_mut(start.offset()..end).ok_or_else(|| {
            MemoryFault::new(
                object,
                EventErrorKind::OutOfBounds {
                    len: buffer_len,
                    end,
                },
            )
        })?;
        range.copy_from_slice(data);
        Ok(())
    }

    pub(crate) fn validate_range(
        &self,
        start: DmaRangeStart,
        length: usize,
    ) -> Result<usize, MemoryFault> {
        let backing = self
            .regions
            .get(start.object().raw())
            .ok_or_else(|| Self::unknown_fault(start.object()))?;
        let end = start
            .offset()
            .checked_add(length)
            .ok_or_else(|| self.range_overflow_fault(start))?;
        backing
            .bytes
            .get(start.offset()..end)
            .map(|_| end)
            .ok_or_else(|| self.out_of_bounds_fault(start.object(), backing.bytes.len(), end))
    }

    pub(crate) fn range_overflow_fault(&self, start: DmaRangeStart) -> MemoryFault {
        let Some(backing) = self.regions.get(start.object().raw()) else {
            return MemoryFault::new(start.object(), EventErrorKind::RangeOverflow);
        };
        let locator = u64::try_from(start.offset())
            .ok()
            .and_then(|offset| backing.region.base().checked_add(offset).ok())
            .filter(|address| *address < backing.region.exclusive_end());
        locator.map_or_else(
            || MemoryFault::new(start.object(), EventErrorKind::RangeOverflow),
            |address| {
                MemoryFault::with_address(
                    start.object(),
                    EventErrorKind::RangeOverflow,
                    fault_address(address),
                )
            },
        )
    }

    /// Reads bytes from one checked IOVA extent.
    pub fn read_iova(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, DmaSpaceError> {
        let (index, start, end) = self.resolve_iova_extent(address, length)?;
        let backing = self.regions.get(index).ok_or(DmaSpaceError::Unmapped)?;
        let range = backing
            .bytes
            .get(start..end)
            .ok_or(DmaSpaceError::CrossesRegion)?;
        Ok(range.to_vec())
    }

    /// Writes bytes into one checked IOVA extent.
    pub fn write_iova(&mut self, address: DmaAddress, data: &[u8]) -> Result<(), DmaSpaceError> {
        let length = u64::try_from(data.len()).map_err(|_| DmaSpaceError::AddressExhausted)?;
        let (index, start, end) = self.resolve_iova_extent(address, length)?;
        let backing = self.regions.get_mut(index).ok_or(DmaSpaceError::Unmapped)?;
        let range = backing
            .bytes
            .get_mut(start..end)
            .ok_or(DmaSpaceError::CrossesRegion)?;
        range.copy_from_slice(data);
        Ok(())
    }

    fn resolve_iova_extent(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> Result<(usize, usize, usize), DmaSpaceError> {
        let end = address
            .checked_add(length)
            .map_err(|_| DmaSpaceError::AddressExhausted)?;
        let index = self
            .regions
            .iter()
            .position(|backing| {
                if backing.region.is_empty() {
                    backing.region.base() == address
                } else {
                    backing.region.base() <= address && address < backing.region.exclusive_end()
                }
            })
            .ok_or(DmaSpaceError::Unmapped)?;
        let backing = self.regions.get(index).ok_or(DmaSpaceError::Unmapped)?;
        if end > backing.region.exclusive_end() {
            return Err(DmaSpaceError::CrossesRegion);
        }
        let start = address
            .raw()
            .checked_sub(backing.region.base().raw())
            .ok_or(DmaSpaceError::Unmapped)?;
        let start = usize::try_from(start).map_err(|_| DmaSpaceError::AddressExhausted)?;
        let length = usize::try_from(length).map_err(|_| DmaSpaceError::StorageExhausted)?;
        let end = start
            .checked_add(length)
            .ok_or(DmaSpaceError::StorageExhausted)?;
        Ok((index, start, end))
    }

    const fn unknown_fault(object: BufferId) -> MemoryFault {
        MemoryFault::new(object, EventErrorKind::UnknownBuffer)
    }

    fn out_of_bounds_fault(&self, object: BufferId, len: usize, end: usize) -> MemoryFault {
        let error_kind = EventErrorKind::OutOfBounds { len, end };
        self.regions.get(object.raw()).map_or_else(
            || MemoryFault::new(object, error_kind),
            |backing| {
                MemoryFault::with_address(
                    object,
                    error_kind,
                    fault_address(backing.region.exclusive_end()),
                )
            },
        )
    }
}
