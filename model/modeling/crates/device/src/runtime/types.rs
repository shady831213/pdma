use crate::AsyncStepStatus;

/// Snapshot of runner-observable hardware progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncRunSnapshot {
    /// Commands retired successfully by the worker loop.
    pub steps: usize,
    /// Model errors observed by the worker loop.
    pub errors: usize,
    /// Stall statuses observed by the worker loop.
    pub stalls: usize,
    /// The status observed on the most recent worker step.
    pub last_status: AsyncStepStatus,
    /// Whether shutdown has been requested.
    pub shutdown_requested: bool,
    pub(super) generation: usize,
}

impl AsyncRunSnapshot {
    /// Returns the publication generation represented by this snapshot.
    pub const fn generation(self) -> usize {
        self.generation
    }
}

/// Final runner summary returned by `AsyncDeviceWorker::run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncRunSummary {
    /// Commands retired successfully by the worker loop.
    pub steps: usize,
    /// Model errors observed by the worker loop.
    pub errors: usize,
    /// Stall statuses observed by the worker loop.
    pub stalls: usize,
    /// The status observed on the most recent worker step.
    pub last_status: AsyncStepStatus,
}
