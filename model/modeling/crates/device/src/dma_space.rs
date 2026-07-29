//! Reset-persistent checked DMA allocation, mapping, and backing storage.

mod access;
mod error;
mod stage;
mod validation;

pub use error::DmaSpaceError;
pub use stage::DmaStage;

use pdma_model_contract::abi::{DmaAddress, DmaRegion};
use pdma_model_contract::ids::BufferId;

const FIRST_IOVA: u64 = 0x1_0000;
const IOVA_ALIGNMENT: u64 = 64;

#[derive(Debug)]
struct RegionBacking {
    region: DmaRegion,
    bytes: Vec<u8>,
}

/// Monotonic checked IOVA allocator with committed host backing storage.
#[derive(Debug)]
pub struct DmaSpace {
    regions: Vec<RegionBacking>,
    next_iova: u64,
    generation: u64,
}

impl DmaSpace {
    /// Creates an empty DMA space at the deterministic first IOVA.
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
            next_iova: FIRST_IOVA,
            generation: 0,
        }
    }

    /// Returns the generation advanced by each successful batch commit.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Proposes an aligned allocation batch without changing allocator or backing state.
    pub fn stage(&self, lengths: &[u64]) -> Result<DmaStage, DmaSpaceError> {
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(lengths.len())
            .map_err(|_| DmaSpaceError::StorageExhausted)?;
        let mut next_iova = self.next_iova;
        let first_object = self.regions.len();
        first_object
            .checked_add(lengths.len())
            .ok_or(DmaSpaceError::AddressExhausted)?;
        for (index, length) in lengths.iter().copied().enumerate() {
            let object_raw = first_object
                .checked_add(index)
                .ok_or(DmaSpaceError::AddressExhausted)?;
            let base = align_iova(next_iova)?;
            let region =
                DmaRegion::try_new(BufferId::new(object_raw), DmaAddress::new(base), length)
                    .map_err(|_| DmaSpaceError::AddressExhausted)?;
            next_iova = reserved_end(region)?;
            regions.push(region);
        }
        Ok(DmaStage {
            generation: self.generation,
            next_iova,
            regions,
        })
    }

    /// Installs an entire staged batch or leaves mappings and bytes unchanged.
    pub fn commit(&mut self, stage: DmaStage) -> Result<Vec<DmaRegion>, DmaSpaceError> {
        if stage.generation != self.generation {
            return Err(DmaSpaceError::StaleStage {
                expected_generation: stage.generation,
                actual_generation: self.generation,
            });
        }
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(DmaSpaceError::AddressExhausted)?;
        self.validate_proposal(&stage)?;
        let mut additions = Vec::new();
        additions
            .try_reserve_exact(stage.regions.len())
            .map_err(|_| DmaSpaceError::StorageExhausted)?;
        for region in stage.regions.iter().copied() {
            let length =
                usize::try_from(region.length()).map_err(|_| DmaSpaceError::StorageExhausted)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| DmaSpaceError::StorageExhausted)?;
            bytes.resize(length, 0);
            additions.push(RegionBacking { region, bytes });
        }
        self.regions
            .try_reserve_exact(additions.len())
            .map_err(|_| DmaSpaceError::StorageExhausted)?;
        let committed = stage.regions;
        self.regions.extend(additions);
        self.next_iova = stage.next_iova;
        self.generation = next_generation;
        Ok(committed)
    }

    /// Allocates and commits one region.
    pub fn allocate(&mut self, length: u64) -> Result<DmaRegion, DmaSpaceError> {
        let stage = self.stage(&[length])?;
        let committed = self.commit(stage)?;
        committed
            .first()
            .copied()
            .ok_or(DmaSpaceError::AddressExhausted)
    }

    pub(crate) fn allocate_legacy(&mut self, length: usize) -> BufferId {
        let object = BufferId::new(self.regions.len());
        let Ok(length) = u64::try_from(length) else {
            return object;
        };
        self.allocate(length).map_or(object, DmaRegion::object)
    }

    fn validate_proposal(&self, stage: &DmaStage) -> Result<(), DmaSpaceError> {
        let mut previous_end = self
            .regions
            .last()
            .map(|backing| reserved_end(backing.region))
            .transpose()?
            .unwrap_or(FIRST_IOVA);
        for region in &stage.regions {
            if region.base().raw() < previous_end {
                return Err(DmaSpaceError::OverlappingRegion);
            }
            previous_end = reserved_end(*region)?;
        }
        Ok(())
    }
}

impl Default for DmaSpace {
    fn default() -> Self {
        Self::new()
    }
}

fn align_iova(address: u64) -> Result<u64, DmaSpaceError> {
    address
        .checked_add(IOVA_ALIGNMENT - 1)
        .map(|value| value & !(IOVA_ALIGNMENT - 1))
        .ok_or(DmaSpaceError::AddressExhausted)
}

fn reserved_end(region: DmaRegion) -> Result<u64, DmaSpaceError> {
    region
        .base()
        .raw()
        .checked_add(region.length().max(1))
        .ok_or(DmaSpaceError::AddressExhausted)
}
