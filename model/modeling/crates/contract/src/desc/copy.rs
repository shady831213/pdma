//! Linear and strided copy descriptor schema.

use crate::{
    dma::DmaRangeStart,
    ids::{BufferId, CommandId, CompletionRingId, QueueId},
};

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
    src: DmaRangeStart,
    dst: DmaRangeStart,
    rows: usize,
    row_bytes: usize,
    src_stride: usize,
    dst_stride: usize,
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
        Self {
            src,
            dst,
            rows,
            row_bytes,
            src_stride,
            dst_stride,
        }
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

    /// Number of rows to copy.
    pub const fn rows(self) -> usize {
        self.rows
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

/// External linear copy command with queue and completion routing metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyCommand {
    command_id: CommandId,
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
    request: CopyRequest,
}

impl CopyCommand {
    /// Creates an external copy command.
    pub const fn new(
        command_id: CommandId,
        queue_id: QueueId,
        completion_ring_id: CompletionRingId,
        request: CopyRequest,
    ) -> Self {
        Self {
            command_id,
            queue_id,
            completion_ring_id,
            request,
        }
    }

    /// Command identifier assigned by the driver.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Queue that carried this command.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Completion ring where the result must be published.
    pub const fn completion_ring_id(self) -> CompletionRingId {
        self.completion_ring_id
    }

    /// Copy request payload.
    pub const fn request(self) -> CopyRequest {
        self.request
    }
}

/// External strided copy command with queue and completion routing metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StridedCopyCommand {
    command_id: CommandId,
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
    request: StridedCopyRequest,
}

impl StridedCopyCommand {
    /// Creates an external strided copy command.
    pub const fn new(
        command_id: CommandId,
        queue_id: QueueId,
        completion_ring_id: CompletionRingId,
        request: StridedCopyRequest,
    ) -> Self {
        Self {
            command_id,
            queue_id,
            completion_ring_id,
            request,
        }
    }

    /// Command identifier assigned by the driver.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Queue that carried this command.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Completion ring where the result must be published.
    pub const fn completion_ring_id(self) -> CompletionRingId {
        self.completion_ring_id
    }

    /// Strided copy request payload.
    pub const fn request(self) -> StridedCopyRequest {
        self.request
    }
}
