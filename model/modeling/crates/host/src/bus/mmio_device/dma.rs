use super::super::BusDmaWrite;

impl From<BusDmaWrite> for pdma_model_device::DmaWrite {
    fn from(write: BusDmaWrite) -> Self {
        let (address, bytes) = write.into_parts();
        Self::new(address, bytes)
    }
}
