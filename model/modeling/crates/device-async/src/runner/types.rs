/// Join failure for an async hardware runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncRunJoinError {
    /// The worker task was cancelled before join completed.
    WorkerCancelled,
    /// The worker task panicked before join completed.
    WorkerPanicked,
}

/// Snapshot of runner-observable hardware progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncRunSnapshot {
    /// Commands retired successfully by the worker loop.
    pub steps: usize,
    /// Model errors observed by the worker loop.
    pub errors: usize,
    /// Stall statuses observed by the worker loop.
    pub stalls: usize,
    /// Whether shutdown has been requested.
    pub shutdown_requested: bool,
    pub(super) generation: usize,
}

/// Final runner summary returned by `join`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncRunSummary {
    /// Commands retired successfully by the worker loop.
    pub steps: usize,
    /// Model errors observed by the worker loop.
    pub errors: usize,
    /// Stall statuses observed by the worker loop.
    pub stalls: usize,
}
