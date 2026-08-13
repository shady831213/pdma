use super::{
    MAPPING_ALIGNMENT, PdmaMemoryError, PdmaMemoryMappings, PreparedPdmaMapping, RegionBacking,
};
use hardware_interface_runtime::{MemoryAlignment, MemoryRegion, SharedMemory};
use std::sync::Arc;

impl PreparedPdmaMapping {
    pub(crate) fn runtime_regions(&self) -> &[MemoryRegion] {
        &self.runtime_regions
    }
}

impl PdmaMemoryMappings {
    pub(crate) fn backing_memory(&self) -> Arc<SharedMemory> {
        Arc::clone(&self.memory)
    }

    pub(crate) fn prepare_stage(
        &self,
        stage: &super::PdmaMemoryStage,
    ) -> Result<PreparedPdmaMapping, PdmaMemoryError> {
        self.validate_stage(stage)?;
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
            .map(|region| MemoryRegion::try_new(region.memory_base(), region.length(), alignment))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PdmaMemoryError::InvalidRegion)?;
        let final_count = self
            .regions
            .len()
            .checked_add(stage.regions.len())
            .ok_or(PdmaMemoryError::StorageExhausted)?;
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(final_count)
            .map_err(|_| PdmaMemoryError::StorageExhausted)?;
        for backing in &self.regions {
            regions.push(RegionBacking {
                region: backing.region,
                storage: backing.storage,
            });
        }
        let mut runtime = runtime_regions.iter().copied();
        for region in stage.regions.iter().copied() {
            let storage = if region.is_empty() {
                None
            } else {
                runtime.next()
            };
            regions.push(RegionBacking { region, storage });
        }
        Ok(PreparedPdmaMapping {
            regions,
            next_address: stage.next_address,
            next_generation,
            runtime_regions,
        })
    }

    pub(crate) fn install_mapping(&mut self, prepared: PreparedPdmaMapping) {
        self.regions = prepared.regions;
        self.next_address = prepared.next_address;
        self.generation = prepared.next_generation;
    }
}
