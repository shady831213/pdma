//! Broadcast copy descriptor schema.

use crate::semantic::ids::BufferId;

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
