//! Checked buffer memory model for PDMA-visible external memory.

use crate::ids::BufferId;
use crate::{ModelError, ModelResult};

/// In-memory collection of typed PDMA buffers.
#[derive(Debug, Default, Clone)]
pub struct Memory {
    buffers: Vec<Vec<u8>>,
}

impl Memory {
    /// Allocates a zero-filled buffer and returns its typed handle.
    pub fn allocate(&mut self, len: usize) -> BufferId {
        let id = BufferId::new(self.buffers.len());
        self.buffers.push(vec![0; len]);
        id
    }

    /// Reads a checked range into a new byte vector.
    pub fn read_checked(&self, id: BufferId, offset: usize, len: usize) -> ModelResult<Vec<u8>> {
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

    /// Writes a checked byte slice into a buffer range.
    pub fn write_checked(&mut self, id: BufferId, offset: usize, data: &[u8]) -> ModelResult<()> {
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
        if let Some(dst) = buffer.get_mut(offset..end) {
            dst.copy_from_slice(data);
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

#[cfg(test)]
mod tests {
    use super::Memory;
    use crate::ModelError;

    #[test]
    fn reads_and_writes_only_checked_ranges() {
        // Given: a four-byte buffer.
        let mut memory = Memory::default();
        let buffer = memory.allocate(4);

        // When: two bytes are written inside the buffer.
        memory
            .write_checked(buffer, 1, &[7, 8])
            .expect("range is valid");

        // Then: the checked read observes only the written range.
        assert_eq!(memory.read_checked(buffer, 0, 4), Ok(vec![0, 7, 8, 0]));
        assert!(matches!(
            memory.write_checked(buffer, 3, &[1, 2]),
            Err(ModelError::OutOfBounds { .. })
        ));
    }
}
