//! External command descriptors and internal scheduled work descriptors.

pub use crate::broadcast::{BroadcastCopyCommand, BroadcastCopyRequest, BroadcastDestinationRange};
use crate::ids::{BufferId, CommandId, CompletionRingId, QueueId};
pub use crate::scheduled::{CopyChunk, ScheduledCopy, ScheduledWork};
pub use crate::vector_add::{
    ScheduledVectorAdd, VectorAddCommand, VectorAddRequest, VectorBufferRange,
};

/// SDK/driver-facing linear copy request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyRequest {
    src: BufferId,
    dst: BufferId,
    len: usize,
}

/// SDK/driver-facing 2D strided copy request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StridedCopyRequest {
    src: BufferId,
    dst: BufferId,
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
        self.src
    }

    /// Destination buffer handle.
    pub const fn dst(self) -> BufferId {
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

impl CopyRequest {
    /// Creates a linear copy request.
    pub const fn new(src: BufferId, dst: BufferId, len: usize) -> Self {
        Self { src, dst, len }
    }

    /// Source buffer handle.
    pub const fn src(self) -> BufferId {
        self.src
    }

    /// Destination buffer handle.
    pub const fn dst(self) -> BufferId {
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

/// Hardware-visible command descriptor emitted by the driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalCommand {
    /// Linear memory-to-memory copy.
    Copy(CopyCommand),
    /// 2D strided memory-to-memory copy.
    StridedCopy(StridedCopyCommand),
    /// One source range copied to multiple destination ranges.
    BroadcastCopy(BroadcastCopyCommand),
    /// Byte-wise wrapping vector add over two source ranges.
    VectorAdd(VectorAddCommand),
}

/// External linear copy command with queue and completion routing metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyCommand {
    command_id: CommandId,
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
    request: CopyRequest,
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
