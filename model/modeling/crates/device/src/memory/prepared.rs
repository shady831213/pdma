use super::{
    MAPPING_ALIGNMENT, PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryStage, RegionBacking,
    runtime_start,
};
use hardware_interface_runtime::{MemoryAlignment, MemoryRegion};

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
