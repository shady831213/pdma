use super::{MAPPING_ALIGNMENT, PdmaMemoryError, PdmaMemoryMappings, reserved_end};
use crate::memory::MappedDmaRegion;

impl PdmaMemoryMappings {
    /// Validates that a region set is aligned, exactly mapped, and pairwise disjoint.
    pub fn validate_region_set(&self, regions: &[MappedDmaRegion]) -> Result<(), PdmaMemoryError> {
        for (index, left) in regions.iter().copied().enumerate() {
            let left_end = reserved_end(left)?;
            for right in regions.iter().copied().skip(index.saturating_add(1)) {
                let right_end = reserved_end(right)?;
                if left.base().raw() < right_end && right.base().raw() < left_end {
                    return Err(PdmaMemoryError::OverlappingRegion);
                }
            }
        }
        for region in regions {
            if region.base().raw() % MAPPING_ALIGNMENT != 0
                || self.region_for_object(region.object()) != Some(*region)
            {
                return Err(PdmaMemoryError::InvalidRegion);
            }
        }
        Ok(())
    }
}
