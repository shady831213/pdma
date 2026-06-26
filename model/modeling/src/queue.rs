//! Queue configuration and directory records.

use crate::ids::{CompletionRingId, QueueId};

/// Software owner identity for a configured queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareOwnerId(usize);

impl SoftwareOwnerId {
    /// Creates a software owner ID.
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Raw owner index.
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Event routing policy for a configured queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventRouting {
    /// Events are routed by queue ID through the device event path.
    QueueLocal,
}

/// Software-provided queue configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueConfig {
    command_depth: usize,
    completion_depth: usize,
    event_routing: EventRouting,
    software_owner: SoftwareOwnerId,
}

impl QueueConfig {
    /// Creates a queue configuration.
    pub const fn new(
        command_depth: usize,
        completion_depth: usize,
        event_routing: EventRouting,
        software_owner: SoftwareOwnerId,
    ) -> Self {
        Self {
            command_depth,
            completion_depth,
            event_routing,
            software_owner,
        }
    }

    /// Command ring depth.
    pub const fn command_depth(self) -> usize {
        self.command_depth
    }

    /// Completion ring depth.
    pub const fn completion_depth(self) -> usize {
        self.completion_depth
    }

    /// Event routing policy.
    pub const fn event_routing(self) -> EventRouting {
        self.event_routing
    }

    /// Software owner identity.
    pub const fn software_owner(self) -> SoftwareOwnerId {
        self.software_owner
    }
}

/// Device queue directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueDirectoryEntry {
    queue_id: QueueId,
    completion_ring_id: CompletionRingId,
    config: QueueConfig,
}

impl QueueDirectoryEntry {
    /// Creates a queue directory entry.
    pub(crate) const fn new(
        queue_id: QueueId,
        completion_ring_id: CompletionRingId,
        config: QueueConfig,
    ) -> Self {
        Self {
            queue_id,
            completion_ring_id,
            config,
        }
    }

    /// Queue ID assigned by the directory.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Completion ring bound to this queue.
    pub const fn completion_ring_id(self) -> CompletionRingId {
        self.completion_ring_id
    }

    /// Original queue configuration.
    pub const fn config(self) -> QueueConfig {
        self.config
    }
}

/// Driver-visible handle to a configured queue directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueConfigHandle {
    entry: QueueDirectoryEntry,
}

impl QueueConfigHandle {
    /// Creates a queue configuration handle from a directory entry.
    pub(crate) const fn new(entry: QueueDirectoryEntry) -> Self {
        Self { entry }
    }

    /// Directory entry backing this handle.
    pub const fn directory_entry(self) -> QueueDirectoryEntry {
        self.entry
    }

    /// Queue ID assigned by the directory.
    pub const fn queue_id(self) -> QueueId {
        self.entry.queue_id()
    }

    /// Completion ring bound to this queue.
    pub const fn completion_ring_id(self) -> CompletionRingId {
        self.entry.completion_ring_id()
    }

    /// Original queue configuration.
    pub const fn config(self) -> QueueConfig {
        self.entry.config()
    }

    /// Command ring depth.
    pub const fn command_depth(self) -> usize {
        self.config().command_depth()
    }

    /// Completion ring depth.
    pub const fn completion_depth(self) -> usize {
        self.config().completion_depth()
    }

    /// Event routing policy.
    pub const fn event_routing(self) -> EventRouting {
        self.config().event_routing()
    }

    /// Software owner identity.
    pub const fn software_owner(self) -> SoftwareOwnerId {
        self.config().software_owner()
    }
}
