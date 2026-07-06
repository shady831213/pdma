//! Internal scheduler output descriptors.

use crate::vector_add::ScheduledVectorAdd;
use pdma_model_contract::ids::{BufferId, CommandId, CompletionRingId, QueueId};

/// One row-level copy chunk emitted by behavioral lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyChunk {
    src: BufferId,
    dst: BufferId,
    src_offset: usize,
    dst_offset: usize,
    len: usize,
}

impl CopyChunk {
    /// Creates one row-level copy chunk.
    pub const fn new(
        src: BufferId,
        dst: BufferId,
        src_offset: usize,
        dst_offset: usize,
        len: usize,
    ) -> Self {
        Self {
            src,
            dst,
            src_offset,
            dst_offset,
            len,
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

    /// Source byte offset.
    pub const fn src_offset(self) -> usize {
        self.src_offset
    }

    /// Destination byte offset.
    pub const fn dst_offset(self) -> usize {
        self.dst_offset
    }

    /// Number of bytes copied by this chunk.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Returns true when this chunk moves zero bytes.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// Internal scheduler output for a deterministic copy engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledCopy {
    queue_id: QueueId,
    command_id: CommandId,
    completion_ring_id: CompletionRingId,
    chunks: Vec<CopyChunk>,
}

impl ScheduledCopy {
    /// Creates an internal scheduled copy descriptor.
    pub const fn new(
        queue_id: QueueId,
        command_id: CommandId,
        completion_ring_id: CompletionRingId,
        chunks: Vec<CopyChunk>,
    ) -> Self {
        Self {
            queue_id,
            command_id,
            completion_ring_id,
            chunks,
        }
    }

    /// Queue that carried the external command.
    pub const fn queue_id(&self) -> QueueId {
        self.queue_id
    }

    /// Command identifier assigned by the driver.
    pub const fn command_id(&self) -> CommandId {
        self.command_id
    }

    /// Completion ring where the result must be published.
    pub const fn completion_ring_id(&self) -> CompletionRingId {
        self.completion_ring_id
    }

    /// Row-level chunks emitted by scheduler lowering.
    pub fn chunks(&self) -> &[CopyChunk] {
        &self.chunks
    }
}

/// Internal scheduled work accepted by device engines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledWork {
    /// Deterministic copy-engine work.
    Copy(ScheduledCopy),
    /// Deterministic vector-add-engine work.
    VectorAdd(ScheduledVectorAdd),
}

impl ScheduledWork {
    /// Queue that carried the external command.
    pub const fn queue_id(&self) -> QueueId {
        match self {
            Self::Copy(copy) => copy.queue_id(),
            Self::VectorAdd(vector_add) => vector_add.queue_id(),
        }
    }

    /// Command identifier assigned by the driver.
    pub const fn command_id(&self) -> CommandId {
        match self {
            Self::Copy(copy) => copy.command_id(),
            Self::VectorAdd(vector_add) => vector_add.command_id(),
        }
    }

    /// Completion ring where the result must be published.
    pub const fn completion_ring_id(&self) -> CompletionRingId {
        match self {
            Self::Copy(copy) => copy.completion_ring_id(),
            Self::VectorAdd(vector_add) => vector_add.completion_ring_id(),
        }
    }
}
