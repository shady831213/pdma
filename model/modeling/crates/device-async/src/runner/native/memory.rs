use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::ids::BufferId;

#[derive(Debug, Default)]
pub(super) struct NativeMemory {
    buffers: Vec<Vec<u8>>,
}

impl NativeMemory {
    pub(super) fn allocate(&mut self, len: usize) -> BufferId {
        let id = BufferId::new(self.buffers.len());
        self.buffers.push(vec![0; len]);
        id
    }

    pub(super) fn read_checked(
        &self,
        id: BufferId,
        offset: usize,
        len: usize,
    ) -> ModelResult<Vec<u8>> {
        let buffer = self.buffer(id)?;
        let end = offset.checked_add(len).ok_or(ModelError::RangeOverflow)?;
        if end > buffer.len() {
            return Err(ModelError::OutOfBounds {
                buffer: id,
                len: buffer.len(),
                end,
            });
        }
        let Some(range) = buffer.get(offset..end) else {
            return Err(ModelError::OutOfBounds {
                buffer: id,
                len: buffer.len(),
                end,
            });
        };
        Ok(range.to_vec())
    }

    pub(super) fn write_checked(
        &mut self,
        id: BufferId,
        offset: usize,
        data: &[u8],
    ) -> ModelResult<()> {
        let buffer = self.buffer_mut(id)?;
        let end = offset
            .checked_add(data.len())
            .ok_or(ModelError::RangeOverflow)?;
        if end > buffer.len() {
            return Err(ModelError::OutOfBounds {
                buffer: id,
                len: buffer.len(),
                end,
            });
        }
        if let Some(range) = buffer.get_mut(offset..end) {
            range.copy_from_slice(data);
        }
        Ok(())
    }

    fn buffer(&self, id: BufferId) -> ModelResult<&[u8]> {
        self.buffers
            .get(id.raw())
            .map(Vec::as_slice)
            .ok_or(ModelError::UnknownBuffer(id))
    }

    fn buffer_mut(&mut self, id: BufferId) -> ModelResult<&mut [u8]> {
        self.buffers
            .get_mut(id.raw())
            .map(Vec::as_mut_slice)
            .ok_or(ModelError::UnknownBuffer(id))
    }
}
