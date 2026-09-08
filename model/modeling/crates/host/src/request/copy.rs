//! Linear and strided copy software requests.

use pdma_model_contract::semantic::{dma::DmaRangeStart, ids::BufferId};
use std::num::NonZeroUsize;

/// SDK/driver-facing linear copy request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyRequest {
    src: DmaRangeStart,
    dst: DmaRangeStart,
    len: usize,
}

impl CopyRequest {
    /// Creates a linear copy request.
    pub const fn new(src: BufferId, dst: BufferId, len: usize) -> Self {
        Self::with_starts(DmaRangeStart::new(src, 0), DmaRangeStart::new(dst, 0), len)
    }

    /// Creates a linear copy request with explicit range starts.
    pub const fn with_starts(src: DmaRangeStart, dst: DmaRangeStart, len: usize) -> Self {
        Self { src, dst, len }
    }

    /// Source buffer handle.
    pub const fn src(self) -> BufferId {
        self.src.object()
    }

    /// Destination buffer handle.
    pub const fn dst(self) -> BufferId {
        self.dst.object()
    }

    /// Source DMA range start.
    pub const fn src_start(self) -> DmaRangeStart {
        self.src
    }

    /// Destination DMA range start.
    pub const fn dst_start(self) -> DmaRangeStart {
        self.dst
    }

    /// Number of bytes to copy.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Returns true when the copy request moves zero bytes.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// SDK/driver-facing 2D strided copy request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StridedCopyRequest {
    rows: StridedRows,
    row_bytes: usize,
    src_stride: usize,
    dst_stride: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StridedRows {
    Empty,
    NonEmpty {
        src: DmaRangeStart,
        dst: DmaRangeStart,
        count: NonZeroUsize,
    },
}

impl StridedCopyRequest {
    /// Creates a 2D strided copy request.
    pub const fn new(
        src: BufferId,
        dst: BufferId,
        rows: usize,
        row_bytes: usize,
        src_stride: usize,
        dst_stride: usize,
    ) -> Self {
        Self::with_starts(
            DmaRangeStart::new(src, 0),
            DmaRangeStart::new(dst, 0),
            rows,
            row_bytes,
            src_stride,
            dst_stride,
        )
    }

    /// Creates a 2D strided copy request with explicit range starts.
    pub const fn with_starts(
        src: DmaRangeStart,
        dst: DmaRangeStart,
        rows: usize,
        row_bytes: usize,
        src_stride: usize,
        dst_stride: usize,
    ) -> Self {
        let Some(count) = NonZeroUsize::new(rows) else {
            return Self::empty(row_bytes, src_stride, dst_stride);
        };
        Self {
            rows: StridedRows::NonEmpty { src, dst, count },
            row_bytes,
            src_stride,
            dst_stride,
        }
    }

    /// Creates a zero-row request without inventing unused DMA operands.
    pub const fn empty(row_bytes: usize, src_stride: usize, dst_stride: usize) -> Self {
        Self {
            rows: StridedRows::Empty,
            row_bytes,
            src_stride,
            dst_stride,
        }
    }

    /// Source buffer handle.
    pub const fn src(self) -> Option<BufferId> {
        match self.rows {
            StridedRows::Empty => None,
            StridedRows::NonEmpty { src, .. } => Some(src.object()),
        }
    }

    /// Destination buffer handle.
    pub const fn dst(self) -> Option<BufferId> {
        match self.rows {
            StridedRows::Empty => None,
            StridedRows::NonEmpty { dst, .. } => Some(dst.object()),
        }
    }

    /// Source DMA range start.
    pub const fn src_start(self) -> Option<DmaRangeStart> {
        match self.rows {
            StridedRows::Empty => None,
            StridedRows::NonEmpty { src, .. } => Some(src),
        }
    }

    /// Destination DMA range start.
    pub const fn dst_start(self) -> Option<DmaRangeStart> {
        match self.rows {
            StridedRows::Empty => None,
            StridedRows::NonEmpty { dst, .. } => Some(dst),
        }
    }

    /// Number of rows to copy.
    pub const fn rows(self) -> usize {
        match self.rows {
            StridedRows::Empty => 0,
            StridedRows::NonEmpty { count, .. } => count.get(),
        }
    }

    /// Number of payload bytes copied from each row.
    pub const fn row_bytes(self) -> usize {
        self.row_bytes
    }

    /// Source byte distance between adjacent row starts.
    pub const fn src_stride(self) -> usize {
        self.src_stride
    }

    /// Destination byte distance between adjacent row starts.
    pub const fn dst_stride(self) -> usize {
        self.dst_stride
    }
}

#[cfg(test)]
mod tests {
    use super::StridedCopyRequest;

    #[test]
    fn empty_strided_request_has_no_meaningless_dma_ranges() {
        // Given: a zero-row semantic strided-copy request.
        let request = StridedCopyRequest::empty(4, 8, 16);

        // When/Then: no source or destination range is invented for unused operands.
        assert_eq!(request.rows(), 0);
        assert_eq!(request.src_start(), None);
        assert_eq!(request.dst_start(), None);
    }
}
