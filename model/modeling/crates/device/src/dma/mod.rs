//! Reset-persistent PDMA memory mappings and checked DMA access.

mod access;
mod error;
mod prepared;
mod region;
mod stage;
mod validation;

pub use error::PdmaMemoryError;
pub use region::MappedDmaRegion;
pub use stage::PdmaMemoryStage;

use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::ids::BufferId;
use std::sync::Arc;

const FIRST_MEMORY_ADDRESS: u64 = 0x1_0000;
const MAPPING_ALIGNMENT: u64 = 64;

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
    storage: Option<MemoryRegion>,
}

pub(crate) struct PreparedPdmaMapping {
    regions: Vec<RegionBacking>,
    next_address: u64,
    next_generation: u64,
    runtime_regions: Vec<MemoryRegion>,
}

/// Monotonic checked memory-mapping allocator over injected runtime storage.
#[derive(Debug)]
pub struct PdmaMemoryMappings {
    regions: Vec<RegionBacking>,
    next_address: u64,
    generation: u64,
    memory: Arc<SharedMemory>,
}

impl PdmaMemoryMappings {
    /// Creates an empty mapping table at the deterministic first address.
    pub const fn new(memory: Arc<SharedMemory>) -> Self {
        Self {
            regions: Vec::new(),
            next_address: FIRST_MEMORY_ADDRESS,
            generation: 0,
            memory,
        }
    }

    pub(crate) fn shared_memory(&self) -> Arc<SharedMemory> {
        Arc::clone(&self.memory)
    }

    /// Returns the generation advanced by each successful batch commit.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Proposes an aligned allocation batch without changing allocator or backing state.
    pub fn stage(&self, lengths: &[u64]) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(lengths.len())
            .map_err(|_| PdmaMemoryError::StorageExhausted)?;
        let mut next_address = self.next_address;
        let first_object = self.regions.len();
        first_object
            .checked_add(lengths.len())
            .ok_or(PdmaMemoryError::AddressExhausted)?;
        for (index, length) in lengths.iter().copied().enumerate() {
            let object_raw = first_object
                .checked_add(index)
                .ok_or(PdmaMemoryError::AddressExhausted)?;
            let base = align_address(next_address)?;
            let region = MappedDmaRegion::try_new(
                BufferId::new(object_raw),
                pdma_model_contract::abi::DmaAddress::new(base),
                length,
            )
            .map_err(|_| PdmaMemoryError::AddressExhausted)?;
            next_address = reserved_end(region)?;
            regions.push(region);
        }
        Ok(PdmaMemoryStage {
            generation: self.generation,
            next_address,
            regions,
        })
    }

    /// Installs an entire staged batch or leaves mappings and bytes unchanged.
    pub fn commit(
        &mut self,
        stage: PdmaMemoryStage,
    ) -> Result<Vec<MappedDmaRegion>, PdmaMemoryError> {
        self.validate_stage(&stage)?;
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(PdmaMemoryError::AddressExhausted)?;
        let alignment = MemoryAlignment::try_new(MAPPING_ALIGNMENT)
            .map_err(|_| PdmaMemoryError::InvalidRegion)?;
        let runtime_regions = stage
            .regions
            .iter()
            .filter(|region| !region.is_empty())
            .map(|region| MemoryRegion::try_new(runtime_start(*region), region.length(), alignment))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PdmaMemoryError::InvalidRegion)?;
        let prepared = self
            .memory
            .prepare_regions(&runtime_regions)
            .map_err(|error| PdmaMemoryError::from_runtime_memory(&error))?;
        let mut additions = Vec::new();
        additions
            .try_reserve_exact(stage.regions.len())
            .map_err(|_| PdmaMemoryError::StorageExhausted)?;
        let mut runtime = runtime_regions.iter().copied();
        for region in stage.regions.iter().copied() {
            let storage = if region.is_empty() {
                None
            } else {
                runtime.next()
            };
            additions.push(RegionBacking { region, storage });
        }
        self.regions
            .try_reserve_exact(additions.len())
            .map_err(|_| PdmaMemoryError::StorageExhausted)?;
        prepared
            .try_install(&self.memory)
            .map_err(|error| PdmaMemoryError::from_runtime_memory(&error))?;
        let committed = stage.regions;
        self.regions.extend(additions);
        self.next_address = stage.next_address;
        self.generation = next_generation;
        Ok(committed)
    }

    pub(crate) fn validate_stage(&self, stage: &PdmaMemoryStage) -> Result<(), PdmaMemoryError> {
        if stage.generation != self.generation {
            return Err(PdmaMemoryError::StaleStage {
                expected_generation: stage.generation,
                actual_generation: self.generation,
            });
        }
        self.validate_proposal(stage)
    }

    /// Allocates and commits one region.
    pub fn allocate(&mut self, length: u64) -> Result<MappedDmaRegion, PdmaMemoryError> {
        let stage = self.stage(&[length])?;
        let committed = self.commit(stage)?;
        committed
            .first()
            .copied()
            .ok_or(PdmaMemoryError::AddressExhausted)
    }

    fn validate_proposal(&self, stage: &PdmaMemoryStage) -> Result<(), PdmaMemoryError> {
        let mut previous_end = self
            .regions
            .last()
            .map(|backing| reserved_end(backing.region))
            .transpose()?
            .unwrap_or(FIRST_MEMORY_ADDRESS);
        for region in &stage.regions {
            if region.base().raw() < previous_end {
                return Err(PdmaMemoryError::OverlappingRegion);
            }
            previous_end = reserved_end(*region)?;
        }
        Ok(())
    }
}

fn align_address(address: u64) -> Result<u64, PdmaMemoryError> {
    address
        .checked_add(MAPPING_ALIGNMENT - 1)
        .map(|value| value & !(MAPPING_ALIGNMENT - 1))
        .ok_or(PdmaMemoryError::AddressExhausted)
}

fn reserved_end(region: MappedDmaRegion) -> Result<u64, PdmaMemoryError> {
    region
        .base()
        .raw()
        .checked_add(region.length().max(1))
        .ok_or(PdmaMemoryError::AddressExhausted)
}
