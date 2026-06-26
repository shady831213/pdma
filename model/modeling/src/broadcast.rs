//! Broadcast copy descriptors.

use crate::ids::{BufferId, CommandId, CompletionRingId, QueueId};

/// One destination range for a broadcast copy request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcastDestinationRange {
    dst: BufferId,
    dst_offset: usize,
}

impl BroadcastDestinationRange {
    /// Creates one broadcast destination range.
    pub const fn new(dst: BufferId, dst_offset: usize) -> Self {
        Self { dst, dst_offset }
    }

    /// Destination buffer handle.
    pub const fn dst(self) -> BufferId {
        self.dst
    }

    /// Destination byte offset.
    pub const fn dst_offset(self) -> usize {
        self.dst_offset
    }
}

/// SDK/driver-facing broadcast copy request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastCopyRequest {
    src: BufferId,
    src_offset: usize,
    len: usize,
    destinations: Vec<BroadcastDestinationRange>,
}

impl BroadcastCopyRequest {
    /// Creates a broadcast copy request.
    pub const fn new(
        src: BufferId,
        src_offset: usize,
        len: usize,
        destinations: Vec<BroadcastDestinationRange>,
    ) -> Self {
        Self {
            src,
            src_offset,
            len,
            destinations,
        }
    }

    /// Source buffer handle.
    pub const fn src(&self) -> BufferId {
        self.src
    }

    /// Source byte offset.
    pub const fn src_offset(&self) -> usize {
        self.src_offset
    }

    /// Number of bytes copied to every destination.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Destination ranges receiving the source payload.
    pub fn destinations(&self) -> &[BroadcastDestinationRange] {
        &self.destinations
    }

    /// Returns true when this broadcast copies zero bytes.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// External broadcast copy command with queue and completion routing metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastCopyCommand {
    command_id: CommandId,
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
    request: BroadcastCopyRequest,
}

impl BroadcastCopyCommand {
    /// Creates an external broadcast copy command.
    pub const fn new(
        command_id: CommandId,
        queue_id: QueueId,
        completion_ring_id: CompletionRingId,
        request: BroadcastCopyRequest,
    ) -> Self {
        Self {
            command_id,
            queue_id,
            completion_ring_id,
            request,
        }
    }

    /// Command identifier assigned by the driver.
    pub const fn command_id(&self) -> CommandId {
        self.command_id
    }

    /// Queue that carried this command.
    pub const fn queue_id(&self) -> QueueId {
        self.queue_id
    }

    /// Completion ring where the result must be published.
    pub const fn completion_ring_id(&self) -> CompletionRingId {
        self.completion_ring_id
    }

    /// Broadcast copy request payload.
    pub const fn request(&self) -> &BroadcastCopyRequest {
        &self.request
    }
}
