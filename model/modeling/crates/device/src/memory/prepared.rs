use super::{
    MAPPING_ALIGNMENT, PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage, PreparedPdmaMapping,
    RegionBacking, runtime_start,
};
use hardware_interface_runtime::{MemoryAlignment, MemoryRegion};

impl PreparedPdmaMapping {
    pub(crate) fn runtime_regions(&self) -> &[MemoryRegion] {
        &self.runtime_regions
    }
}

impl PdmaMemoryMappings {
    pub(super) fn prepare_runtime_regions(
        &self,
        stage: &PdmaMemoryStage,
    ) -> Result<(u64, Vec<MemoryRegion>), PdmaMemoryError> {
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
            .map(|region| MemoryRegion::try_new(runtime_start(*region), region.length(), alignment))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PdmaMemoryError::InvalidRegion)?;
        Ok((next_generation, runtime_regions))
    }

    pub(crate) fn prepare_stage(
        &self,
        stage: &PdmaMemoryStage,
    ) -> Result<PreparedPdmaMapping, PdmaMemoryError> {
        let (next_generation, runtime_regions) = self.prepare_runtime_regions(stage)?;
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
        regions.extend(RegionBacking::for_stage(stage, &runtime_regions));
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

impl RegionBacking {
    pub(super) fn for_stage<'a>(
        stage: &'a PdmaMemoryStage,
        runtime_regions: &'a [MemoryRegion],
    ) -> impl Iterator<Item = Self> + 'a {
        let mut runtime = runtime_regions.iter().copied();
        stage.regions.iter().copied().map(move |region| {
            let storage = if region.is_empty() {
                None
            } else {
                runtime.next()
            };
            Self { region, storage }
        })
    }
}
