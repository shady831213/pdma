//! DMA-visible range vocabulary without runtime address-projection semantics.

use crate::semantic::ids::DmaObjectId;

/// DMA-visible object and byte offset where a range begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DmaRangeStart {
    object: DmaObjectId,
    offset: usize,
}

impl DmaRangeStart {
    /// Creates a range start inside a DMA-visible object.
    pub const fn new(object: DmaObjectId, offset: usize) -> Self {
        Self { object, offset }
    }

    /// DMA-visible object containing the range start.
    pub const fn object(self) -> DmaObjectId {
        self.object
    }

    /// Raw byte offset inside the object.
    pub const fn offset(self) -> usize {
        self.offset
    }
}

/// DMA-visible byte range inside a DMA object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaRange {
    start: DmaRangeStart,
    len: usize,
}

impl DmaRange {
    /// Creates a DMA-visible byte range.
    pub const fn new(start: DmaRangeStart, len: usize) -> Self {
        Self { start, len }
    }

    /// DMA object containing the range.
    pub const fn object(self) -> DmaObjectId {
        self.start.object()
    }

    /// Starting byte offset inside the object.
    pub const fn start(self) -> DmaRangeStart {
        self.start
    }

    /// Starting byte offset inside the object.
    pub const fn offset(self) -> usize {
        self.start.offset()
    }

    /// Number of bytes in the range.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Returns true when the range contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::ids::{BufferId, DmaObjectId};

    use super::{DmaRange, DmaRangeStart};

    #[test]
    fn buffer_id_aliases_dma_object_id_when_constructed() {
        // Given: the v0 buffer alias is used at the public boundary.
        let buffer = BufferId::new(7);

        // When: it is assigned to the new DMA object vocabulary.
        let object: DmaObjectId = buffer;

        // Then: both names expose the same stable raw identifier.
        assert_eq!(object.raw(), 7);
    }

    #[test]
    fn dma_range_exposes_object_start_and_len_when_constructed() {
        // Given: a DMA object range with a non-zero byte start.
        let object = DmaObjectId::new(3);
        let start = DmaRangeStart::new(object, 16);

        // When: the range is constructed for eight bytes.
        let range = DmaRange::new(start, 8);

        // Then: accessors return the contract fields without runtime projection.
        assert_eq!(range.object(), object);
        assert_eq!(range.start().object(), object);
        assert_eq!(range.offset(), 16);
        assert_eq!(range.len(), 8);
        assert!(!range.is_empty());
    }
}
