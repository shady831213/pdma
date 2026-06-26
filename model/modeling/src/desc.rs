//! External command descriptors and internal scheduled work descriptors.

use crate::ids::{BufferId, CommandId, CompletionRingId, QueueId};

/// SDK/driver-facing linear copy request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyRequest {
    src: BufferId,
    dst: BufferId,
    len: usize,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalCommand {
    /// Linear memory-to-memory copy.
    Copy(CopyCommand),
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

/// Internal scheduler output for a deterministic copy engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledCopy {
    command: CopyCommand,
}

impl ScheduledCopy {
    /// Creates an internal scheduled copy descriptor.
    pub const fn new(command: CopyCommand) -> Self {
        Self { command }
    }

    /// External command metadata retained for completion publication.
    pub const fn command(self) -> CopyCommand {
        self.command
    }

    /// Original copy request.
    pub const fn request(self) -> CopyRequest {
        self.command.request()
    }
}
