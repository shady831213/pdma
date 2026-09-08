//! Reset-persistent PDMA mapping metadata and checked external-memory access.

mod access;
mod error;
mod region;
mod validation;

pub use error::PdmaMemoryError;
pub use region::MappedDmaRegion;

use hardware_interface_runtime::{MemoryAddress, SharedMemoryAccess};
use pdma_model_contract::abi::DmaAddress;

pub(crate) const MAPPING_ALIGNMENT: u64 = 64;

pub(crate) const fn runtime_address(address: DmaAddress) -> MemoryAddress {
    MemoryAddress::new(address.raw())
}

const fn runtime_start(region: MappedDmaRegion) -> MemoryAddress {
    runtime_address(region.base())
}

const fn runtime_end(region: MappedDmaRegion) -> MemoryAddress {
    runtime_address(region.exclusive_end())
}

#[derive(Debug)]
struct RegionBacking {
    region: MappedDmaRegion,
}

/// Checked logical mappings over an externally supplied read/write capability.
#[derive(Debug)]
pub struct PdmaMemoryMappings {
    regions: Vec<RegionBacking>,
    generation: u64,
    memory: SharedMemoryAccess,
}

impl PdmaMemoryMappings {
    /// Creates an empty mapping table without allocating backing bytes.
    pub fn new(memory: impl Into<SharedMemoryAccess>) -> Self {
        Self {
            regions: Vec::new(),
            generation: 0,
            memory: memory.into(),
        }
    }

    pub(crate) fn memory_access(&self) -> SharedMemoryAccess {
        self.memory.clone()
    }

    /// Returns the generation advanced by each successful registration batch.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Registers metadata for already provisioned addresses without modifying their bytes.
    pub fn register_regions(&mut self, regions: &[MappedDmaRegion]) -> Result<(), PdmaMemoryError> {
        self.prepare_registration(regions)?;
        self.finish_registration(regions);
        Ok(())
    }

    pub(crate) fn prepare_registration(
        &mut self,
        regions: &[MappedDmaRegion],
    ) -> Result<(), PdmaMemoryError> {
        self.generation
            .checked_add(1)
            .ok_or(PdmaMemoryError::AddressExhausted)?;
        for (index, region) in regions.iter().copied().enumerate() {
            if region.object().raw()
                != self
                    .regions
                    .len()
                    .checked_add(index)
                    .ok_or(PdmaMemoryError::AddressExhausted)?
                || region.base().raw() % MAPPING_ALIGNMENT != 0
            {
                return Err(PdmaMemoryError::InvalidRegion);
            }
            let end = reserved_end(region)?;
            for other in self
                .regions
                .iter()
                .map(|backing| backing.region)
                .chain(regions.iter().copied().take(index))
            {
                if region.base().raw() < reserved_end(other)? && other.base().raw() < end {
                    return Err(PdmaMemoryError::OverlappingRegion);
                }
            }
        }
        self.regions
            .try_reserve_exact(regions.len())
            .map_err(|_| PdmaMemoryError::StorageExhausted)
    }

    pub(crate) fn finish_registration(&mut self, regions: &[MappedDmaRegion]) {
        self.regions.extend(
            regions
                .iter()
                .copied()
                .map(|region| RegionBacking { region }),
        );
        self.generation += 1;
    }
}

pub(crate) fn reserved_end(region: MappedDmaRegion) -> Result<u64, PdmaMemoryError> {
    region
        .base()
        .raw()
        .checked_add(region.length().max(1))
        .ok_or(PdmaMemoryError::AddressExhausted)
}
