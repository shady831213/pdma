use super::{PdmaMemoryError, PdmaMemoryMappings, runtime_address, runtime_end, runtime_start};
use crate::abi::dma::MappedDmaRegion;
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAlignment};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::dma::DmaRangeStart;
use pdma_model_contract::semantic::event::{EventErrorKind, MemoryFault};
use pdma_model_contract::semantic::ids::BufferId;

const BYTE_ALIGNMENT: MemoryAlignment = MemoryAlignment::ONE;

impl PdmaMemoryMappings {
    /// Resolves a semantic buffer identity to logical mapping metadata.
    pub fn region_for_object(&self, object: BufferId) -> Option<MappedDmaRegion> {
        self.regions.get(object.raw()).map(|backing| backing.region)
    }

    /// Resolves an address contained by one non-empty logical mapping.
    pub fn region_containing(&self, address: DmaAddress) -> Option<MappedDmaRegion> {
        let address = runtime_address(address);
        self.regions
            .iter()
            .find(|backing| {
                runtime_start(backing.region) <= address
                    && address.raw() < runtime_end(backing.region).raw()
            })
            .map(|backing| backing.region)
    }

    pub(crate) fn region_ending_at(&self, address: DmaAddress) -> Option<MappedDmaRegion> {
        self.regions
            .iter()
            .find(|backing| backing.region.exclusive_end() == address)
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
    ) -> Result<DmaAddress, MemoryFault> {
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
        runtime_start(backing.region)
            .raw()
            .checked_add(offset)
            .map(DmaAddress::new)
            .ok_or_else(|| self.range_overflow_fault(start))
    }

    pub(crate) fn range_overflow_fault(&self, start: DmaRangeStart) -> MemoryFault {
        let Some(backing) = self.regions.get(start.object().raw()) else {
            return MemoryFault::new(start.object(), EventErrorKind::RangeOverflow);
        };
        let locator = u64::try_from(start.offset())
            .ok()
            .and_then(|offset| runtime_start(backing.region).raw().checked_add(offset))
            .filter(|address| *address < runtime_end(backing.region).raw());
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
        address: DmaAddress,
        length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        let backing = self.resolve_extent(address, length)?;
        if backing.storage.is_none() {
            return Ok(Vec::new());
        }
        let length = usize::try_from(length).map_err(|_| PdmaMemoryError::StorageExhausted)?;
        let mut bytes = vec![0; length];
        self.memory
            .read(runtime_address(address), BYTE_ALIGNMENT, &mut bytes)
            .map_err(map_access_error)?;
        Ok(bytes)
    }

    /// Writes one checked logical extent into the injected store.
    pub fn write_memory(&self, address: DmaAddress, data: &[u8]) -> Result<(), PdmaMemoryError> {
        let length = u64::try_from(data.len()).map_err(|_| PdmaMemoryError::AddressExhausted)?;
        let backing = self.resolve_extent(address, length)?;
        if backing.storage.is_none() {
            return Ok(());
        }
        self.memory
            .write(runtime_address(address), BYTE_ALIGNMENT, data)
            .map_err(map_access_error)
    }

    /// Returns the number of committed logical mappings.
    pub fn mapped_region_count(&self) -> usize {
        self.regions.len()
    }

    fn resolve_extent(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> Result<&RegionBacking, PdmaMemoryError> {
        let runtime = runtime_address(address);
        let backing = self
            .regions
            .iter()
            .find(|backing| {
                if backing.region.is_empty() {
                    runtime_start(backing.region) == runtime
                } else {
                    runtime_start(backing.region) <= runtime
                        && runtime.raw() < runtime_end(backing.region).raw()
                }
            })
            .ok_or(PdmaMemoryError::Unmapped)?;
        let end = address
            .raw()
            .checked_add(length)
            .ok_or(PdmaMemoryError::AddressExhausted)?;
        if end > runtime_end(backing.region).raw() {
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
                        runtime_end(backing.region).raw(),
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
