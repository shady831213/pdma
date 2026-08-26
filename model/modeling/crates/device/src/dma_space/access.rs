use super::{PdmaMemoryError, PdmaMemoryMappings};
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::binding::PdmaMemoryRegion;
use pdma_model_contract::semantic::dma::DmaRangeStart;
use pdma_model_contract::semantic::event::{EventErrorKind, MemoryFault};
use pdma_model_contract::semantic::ids::BufferId;

const BYTE_ALIGNMENT: MemoryAlignment = MemoryAlignment::ONE;

impl PdmaMemoryMappings {
    /// Resolves a semantic buffer identity to logical mapping metadata.
    pub fn region_for_object(&self, object: BufferId) -> Option<PdmaMemoryRegion> {
        self.regions.get(object.raw()).map(|backing| backing.region)
    }

    /// Resolves an address contained by one non-empty logical mapping.
    pub fn region_containing(&self, address: impl Into<MemoryAddress>) -> Option<PdmaMemoryRegion> {
        let address = address.into();
        self.regions
            .iter()
            .find(|backing| {
                backing.region.memory_base() <= address
                    && address.raw() < backing.region.memory_exclusive_end().raw()
            })
            .map(|backing| backing.region)
    }

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
        let address = self.validate_range(start, length)?;
        self.read_memory(
            address,
            u64::try_from(length).map_err(|_| self.range_overflow_fault(start))?,
        )
        .map_err(|_| {
            self.out_of_bounds_fault(start.object(), start.offset().saturating_add(length))
        })
    }

    pub(crate) fn write_object(
        &self,
        object: BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        self.write_range(DmaRangeStart::new(object, offset), data)
            .map_err(|fault| ModelError::from_memory_fault(fault.buffer(), fault.error_kind()))
    }

    pub(crate) fn write_range(&self, start: DmaRangeStart, data: &[u8]) -> Result<(), MemoryFault> {
        let address = self.validate_range(start, data.len())?;
        self.write_memory(address, data).map_err(|_| {
            self.out_of_bounds_fault(start.object(), start.offset().saturating_add(data.len()))
        })
    }

    pub(crate) fn validate_range(
        &self,
        start: DmaRangeStart,
        length: usize,
    ) -> Result<MemoryAddress, MemoryFault> {
        let backing = self
            .regions
            .get(start.object().raw())
            .ok_or_else(|| Self::unknown_fault(start.object()))?;
        let end = start
            .offset()
            .checked_add(length)
            .ok_or_else(|| self.range_overflow_fault(start))?;
        let logical_length = usize::try_from(backing.region.length()).unwrap_or(usize::MAX);
        if end > logical_length {
            return Err(self.out_of_bounds_fault(start.object(), end));
        }
        let offset = u64::try_from(start.offset()).map_err(|_| self.range_overflow_fault(start))?;
        backing
            .region
            .memory_base()
            .raw()
            .checked_add(offset)
            .map(MemoryAddress::new)
            .ok_or_else(|| self.range_overflow_fault(start))
    }

    pub(crate) fn range_overflow_fault(&self, start: DmaRangeStart) -> MemoryFault {
        let Some(backing) = self.regions.get(start.object().raw()) else {
            return MemoryFault::new(start.object(), EventErrorKind::RangeOverflow);
        };
        let locator = u64::try_from(start.offset())
            .ok()
            .and_then(|offset| backing.region.memory_base().raw().checked_add(offset))
            .filter(|address| *address < backing.region.memory_exclusive_end().raw());
        locator.map_or_else(
            || MemoryFault::new(start.object(), EventErrorKind::RangeOverflow),
            |address| {
                MemoryFault::with_address(
                    start.object(),
                    EventErrorKind::RangeOverflow,
                    pdma_model_contract::semantic::FaultAddress::new(address),
                )
            },
        )
    }

    /// Reads one checked logical extent from the injected store.
    pub fn read_memory(
        &self,
        address: impl Into<MemoryAddress>,
        length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        let address = address.into();
        let backing = self.resolve_extent(address, length)?;
        if backing.storage.is_none() {
            return Ok(Vec::new());
        }
        let length = usize::try_from(length).map_err(|_| PdmaMemoryError::StorageExhausted)?;
        let mut bytes = vec![0; length];
        self.memory
            .read(address, BYTE_ALIGNMENT, &mut bytes)
            .map_err(map_access_error)?;
        Ok(bytes)
    }

    /// Writes one checked logical extent into the injected store.
    pub fn write_memory(
        &self,
        address: impl Into<MemoryAddress>,
        data: &[u8],
    ) -> Result<(), PdmaMemoryError> {
        let address = address.into();
        let length = u64::try_from(data.len()).map_err(|_| PdmaMemoryError::AddressExhausted)?;
        let backing = self.resolve_extent(address, length)?;
        if backing.storage.is_none() {
            return Ok(());
        }
        self.memory
            .write(address, BYTE_ALIGNMENT, data)
            .map_err(map_access_error)
    }

    /// Returns the number of committed logical mappings.
    pub fn mapped_region_count(&self) -> usize {
        self.regions.len()
    }

    fn resolve_extent(
        &self,
        address: MemoryAddress,
        length: u64,
    ) -> Result<&RegionBacking, PdmaMemoryError> {
        let backing = self
            .regions
            .iter()
            .find(|backing| {
                if backing.region.is_empty() {
                    backing.region.memory_base() == address
                } else {
                    backing.region.memory_base() <= address
                        && address.raw() < backing.region.memory_exclusive_end().raw()
                }
            })
            .ok_or(PdmaMemoryError::Unmapped)?;
        let end = address
            .raw()
            .checked_add(length)
            .ok_or(PdmaMemoryError::AddressExhausted)?;
        if end > backing.region.memory_exclusive_end().raw() {
            return Err(PdmaMemoryError::CrossesRegion);
        }
        Ok(backing)
    }

    const fn unknown_fault(object: BufferId) -> MemoryFault {
        MemoryFault::new(object, EventErrorKind::UnknownBuffer)
    }

    fn out_of_bounds_fault(&self, object: BufferId, end: usize) -> MemoryFault {
        let len = self
            .regions
            .get(object.raw())
            .and_then(|backing| usize::try_from(backing.region.length()).ok())
            .unwrap_or(0);
        let error_kind = EventErrorKind::OutOfBounds { len, end };
        self.regions.get(object.raw()).map_or_else(
            || MemoryFault::new(object, error_kind),
            |backing| {
                MemoryFault::with_address(
                    object,
                    error_kind,
                    pdma_model_contract::semantic::FaultAddress::new(
                        backing.region.memory_exclusive_end().raw(),
                    ),
                )
            },
        )
    }
}

const fn map_access_error(error: MemoryAccessError) -> PdmaMemoryError {
    match error {
        MemoryAccessError::Unmapped { .. } => PdmaMemoryError::Unmapped,
        MemoryAccessError::CrossesRegion { .. } => PdmaMemoryError::CrossesRegion,
        MemoryAccessError::AddressOverflow { .. } => PdmaMemoryError::AddressExhausted,
        MemoryAccessError::Misaligned { .. } => PdmaMemoryError::InvalidRegion,
    }
}

use super::RegionBacking;
