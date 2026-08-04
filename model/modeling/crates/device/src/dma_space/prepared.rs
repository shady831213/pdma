use super::{DmaSpace, DmaSpaceError, RegionBacking};

impl DmaSpace {
    pub(crate) fn prepare_clone(&self) -> Result<Self, DmaSpaceError> {
        self.try_clone()
    }

    pub(crate) fn install_prepared(&mut self, prepared: Self) {
        *self = prepared;
    }

    fn try_clone(&self) -> Result<Self, DmaSpaceError> {
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(self.regions.len())
            .map_err(|_| DmaSpaceError::StorageExhausted)?;
        for backing in &self.regions {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(backing.bytes.len())
                .map_err(|_| DmaSpaceError::StorageExhausted)?;
            bytes.extend_from_slice(&backing.bytes);
            regions.push(RegionBacking {
                region: backing.region,
                bytes,
            });
        }
        Ok(Self {
            regions,
            next_iova: self.next_iova,
            generation: self.generation,
        })
    }
}
