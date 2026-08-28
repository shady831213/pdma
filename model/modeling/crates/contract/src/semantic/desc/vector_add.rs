//! Vector-add descriptor schema.

use crate::semantic::ids::BufferId;

/// SDK/driver-facing vector-add request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorAddRequest {
    src_a: VectorBufferRange,
    src_b: VectorBufferRange,
    dst: VectorBufferRange,
    len: usize,
}

/// One vector-add buffer range start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorBufferRange {
    buffer: BufferId,
    offset: usize,
}

impl VectorBufferRange {
    /// Creates one vector-add buffer range start.
    pub const fn new(buffer: BufferId, offset: usize) -> Self {
        Self { buffer, offset }
    }

    /// Buffer handle.
    pub const fn buffer(self) -> BufferId {
        self.buffer
    }

    /// Start byte offset.
    pub const fn offset(self) -> usize {
        self.offset
    }
}

impl VectorAddRequest {
    /// Creates a byte-wise wrapping vector-add request.
    pub const fn new(
        src_a: VectorBufferRange,
        src_b: VectorBufferRange,
        dst: VectorBufferRange,
        len: usize,
    ) -> Self {
        Self {
            src_a,
            src_b,
            dst,
            len,
        }
    }

    /// First source buffer handle.
    pub const fn src_a(self) -> BufferId {
        self.src_a.buffer()
    }

    /// Second source buffer handle.
    pub const fn src_b(self) -> BufferId {
        self.src_b.buffer()
    }

    /// Destination buffer handle.
    pub const fn dst(self) -> BufferId {
        self.dst.buffer()
    }

    /// First source byte offset.
    pub const fn src_a_offset(self) -> usize {
        self.src_a.offset()
    }

    /// Second source byte offset.
    pub const fn src_b_offset(self) -> usize {
        self.src_b.offset()
    }

    /// Destination byte offset.
    pub const fn dst_offset(self) -> usize {
        self.dst.offset()
    }

    /// Number of bytes to add.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Returns true when this vector add moves zero bytes.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}
