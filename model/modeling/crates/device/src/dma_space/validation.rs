use super::{DmaSpace, DmaSpaceError, IOVA_ALIGNMENT, reserved_end};
use pdma_model_contract::bridge::DmaRegion;

impl DmaSpace {
    /// Validates that a region set is aligned, exactly mapped, and pairwise disjoint.
    pub fn validate_region_set(&self, regions: &[DmaRegion]) -> Result<(), DmaSpaceError> {
        for (index, left) in regions.iter().copied().enumerate() {
            let left_end = reserved_end(left)?;
            for right in regions.iter().copied().skip(index.saturating_add(1)) {
                let right_end = reserved_end(right)?;
                if left.base().raw() < right_end && right.base().raw() < left_end {
                    return Err(DmaSpaceError::OverlappingRegion);
                }
            }
        }
        for region in regions {
            if region.base().raw() % IOVA_ALIGNMENT != 0
                || self.region_for_object(region.object()) != Some(*region)
            {
                return Err(DmaSpaceError::InvalidRegion);
            }
        }
        Ok(())
    }
}
