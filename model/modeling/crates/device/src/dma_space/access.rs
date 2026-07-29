use super::{DmaSpace, DmaSpaceError};
use pdma_model_contract::abi::{DmaAddress, DmaRegion};
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::ids::BufferId;

impl DmaSpace {
    /// Resolves a semantic DMA object to its committed region.
    pub fn region_for_object(&self, object: BufferId) -> Option<DmaRegion> {
        self.regions.get(object.raw()).map(|backing| backing.region)
    }

    /// Resolves an IOVA to the one committed region containing that byte.
    pub fn region_containing(&self, address: DmaAddress) -> Option<DmaRegion> {
        self.regions
            .iter()
            .find(|backing| {
                backing.region.base() <= address && address < backing.region.exclusive_end()
            })
            .map(|backing| backing.region)
    }

    /// Reads a checked range through a semantic object and platform-sized offset.
    pub(crate) fn read_object(
        &self,
        object: BufferId,
        offset: usize,
        length: usize,
    ) -> ModelResult<Vec<u8>> {
        let backing = self
            .regions
            .get(object.raw())
            .ok_or(ModelError::UnknownBuffer(object))?;
        let end = offset
            .checked_add(length)
            .ok_or(ModelError::RangeOverflow)?;
        let range = backing
            .bytes
            .get(offset..end)
            .ok_or(ModelError::OutOfBounds {
                buffer: object,
                len: backing.bytes.len(),
                end,
            })?;
        Ok(range.to_vec())
    }

    /// Writes a checked range through a semantic object and platform-sized offset.
    pub(crate) fn write_object(
        &mut self,
        object: BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        let backing = self
            .regions
            .get_mut(object.raw())
            .ok_or(ModelError::UnknownBuffer(object))?;
        let end = offset
            .checked_add(data.len())
            .ok_or(ModelError::RangeOverflow)?;
        let buffer_len = backing.bytes.len();
        let range = backing
            .bytes
            .get_mut(offset..end)
            .ok_or(ModelError::OutOfBounds {
                buffer: object,
                len: buffer_len,
                end,
            })?;
        range.copy_from_slice(data);
        Ok(())
    }

    /// Reads bytes from one checked IOVA extent.
    pub fn read_iova(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, DmaSpaceError> {
        let (index, start, end) = self.resolve_iova_extent(address, length)?;
        let backing = self.regions.get(index).ok_or(DmaSpaceError::Unmapped)?;
        let range = backing
            .bytes
            .get(start..end)
            .ok_or(DmaSpaceError::CrossesRegion)?;
        Ok(range.to_vec())
    }

    /// Writes bytes into one checked IOVA extent.
    pub fn write_iova(&mut self, address: DmaAddress, data: &[u8]) -> Result<(), DmaSpaceError> {
        let length = u64::try_from(data.len()).map_err(|_| DmaSpaceError::AddressExhausted)?;
        let (index, start, end) = self.resolve_iova_extent(address, length)?;
        let backing = self.regions.get_mut(index).ok_or(DmaSpaceError::Unmapped)?;
        let range = backing
            .bytes
            .get_mut(start..end)
            .ok_or(DmaSpaceError::CrossesRegion)?;
        range.copy_from_slice(data);
        Ok(())
    }

    fn resolve_iova_extent(
        &self,
        address: DmaAddress,
        length: u64,
    ) -> Result<(usize, usize, usize), DmaSpaceError> {
        let end = address
            .checked_add(length)
            .map_err(|_| DmaSpaceError::AddressExhausted)?;
        let index = self
            .regions
            .iter()
            .position(|backing| {
                if backing.region.is_empty() {
                    backing.region.base() == address
                } else {
                    backing.region.base() <= address && address < backing.region.exclusive_end()
                }
            })
            .ok_or(DmaSpaceError::Unmapped)?;
        let backing = self.regions.get(index).ok_or(DmaSpaceError::Unmapped)?;
        if end > backing.region.exclusive_end() {
            return Err(DmaSpaceError::CrossesRegion);
        }
        let start = address
            .raw()
            .checked_sub(backing.region.base().raw())
            .ok_or(DmaSpaceError::Unmapped)?;
        let start = usize::try_from(start).map_err(|_| DmaSpaceError::AddressExhausted)?;
        let length = usize::try_from(length).map_err(|_| DmaSpaceError::StorageExhausted)?;
        let end = start
            .checked_add(length)
            .ok_or(DmaSpaceError::StorageExhausted)?;
        Ok((index, start, end))
    }
}
