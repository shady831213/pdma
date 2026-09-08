use std::sync::Arc;

use hardware_interface_runtime::{MemoryAlignment, MemoryRegion, SharedMemory, SharedMemoryAccess};
use pdma_model_contract::{abi::DmaAddress, semantic::BufferId};

use crate::memory::{
    MAPPING_ALIGNMENT, MappedDmaRegion, PdmaMemoryError, PdmaMemoryMappings, reserved_end,
    runtime_address,
};

const FIRST_MEMORY_ADDRESS: u64 = 0x1_0000;

/// Read-only proposal for an atomic runtime DMA allocation batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdmaMemoryStage {
    generation: u64,
    next_address: u64,
    regions: Vec<MappedDmaRegion>,
}

impl PdmaMemoryStage {
    /// Returns the mapping generation observed when staging.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Returns proposed regions without installing them.
    pub fn regions(&self) -> &[MappedDmaRegion] {
        &self.regions
    }
}

/// Runtime-owned DMA allocation policy and backend provisioning authority.
#[derive(Debug)]
pub struct PdmaMemoryProvisioning {
    access: SharedMemoryAccess,
    storage: Option<Arc<SharedMemory>>,
    next_address: u64,
}

impl PdmaMemoryProvisioning {
    /// Selects shared in-process storage and its atomic installation capability.
    pub fn shared(memory: Arc<SharedMemory>) -> Self {
        Self {
            access: memory.clone().into(),
            storage: Some(memory),
            next_address: FIRST_MEMORY_ADDRESS,
        }
    }

    /// Selects externally provisioned storage whose addresses are registered separately.
    pub const fn external(access: SharedMemoryAccess) -> Self {
        Self {
            access,
            storage: None,
            next_address: FIRST_MEMORY_ADDRESS,
        }
    }

    /// Returns the read/write capability supplied to both host and device.
    pub fn memory_access(&self) -> SharedMemoryAccess {
        self.access.clone()
    }

    pub(crate) fn validate_binding(
        &self,
        mappings: &PdmaMemoryMappings,
    ) -> Result<(), PdmaMemoryError> {
        if self.access.shares_backend(&mappings.memory_access()) {
            Ok(())
        } else {
            Err(PdmaMemoryError::InvalidRegion)
        }
    }

    /// Proposes aligned addresses without changing mappings or backing storage.
    pub fn stage(
        &self,
        mappings: &PdmaMemoryMappings,
        lengths: &[u64],
    ) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        self.validate_binding(mappings)?;
        if self.storage.is_none() {
            return Err(PdmaMemoryError::InvalidRegion);
        }
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(lengths.len())
            .map_err(|_| PdmaMemoryError::StorageExhausted)?;
        let mut next_address = self.next_address;
        for (index, length) in lengths.iter().copied().enumerate() {
            let object = mappings
                .mapped_region_count()
                .checked_add(index)
                .ok_or(PdmaMemoryError::AddressExhausted)?;
            let base = next_address
                .checked_add(MAPPING_ALIGNMENT - 1)
                .map(|value| value & !(MAPPING_ALIGNMENT - 1))
                .ok_or(PdmaMemoryError::AddressExhausted)?;
            let region =
                MappedDmaRegion::try_new(BufferId::new(object), DmaAddress::new(base), length)
                    .map_err(|_| PdmaMemoryError::AddressExhausted)?;
            next_address = reserved_end(region)?;
            regions.push(region);
        }
        Ok(PdmaMemoryStage {
            generation: mappings.generation(),
            next_address,
            regions,
        })
    }

    /// Installs all staged backing regions before publishing their mapping metadata.
    pub fn commit(
        &mut self,
        mappings: &mut PdmaMemoryMappings,
        stage: PdmaMemoryStage,
    ) -> Result<Vec<MappedDmaRegion>, PdmaMemoryError> {
        self.validate_binding(mappings)?;
        if stage.generation != mappings.generation() {
            return Err(PdmaMemoryError::StaleStage {
                expected_generation: stage.generation,
                actual_generation: mappings.generation(),
            });
        }
        let storage = self
            .storage
            .as_ref()
            .ok_or(PdmaMemoryError::InvalidRegion)?;
        let alignment = MemoryAlignment::try_new(MAPPING_ALIGNMENT)
            .map_err(|_| PdmaMemoryError::InvalidRegion)?;
        let runtime_regions = stage
            .regions
            .iter()
            .filter(|region| !region.is_empty())
            .map(|region| {
                MemoryRegion::try_new(runtime_address(region.base()), region.length(), alignment)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PdmaMemoryError::InvalidRegion)?;
        mappings.prepare_registration(&stage.regions)?;
        let prepared = storage
            .prepare_regions(&runtime_regions)
            .map_err(|error| PdmaMemoryError::from_runtime_memory(&error))?;
        prepared
            .try_install(storage)
            .map_err(|error| PdmaMemoryError::from_runtime_memory(&error))?;
        mappings.finish_registration(&stage.regions);
        self.next_address = stage.next_address;
        Ok(stage.regions)
    }

    /// Allocates and commits one region through the selected runtime storage.
    pub fn allocate(
        &mut self,
        mappings: &mut PdmaMemoryMappings,
        length: u64,
    ) -> Result<MappedDmaRegion, PdmaMemoryError> {
        let stage = self.stage(mappings, &[length])?;
        self.commit(mappings, stage)?
            .first()
            .copied()
            .ok_or(PdmaMemoryError::AddressExhausted)
    }
}
