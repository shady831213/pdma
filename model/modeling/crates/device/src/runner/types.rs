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

/// Status reported by the most recent async runner step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncStepStatus {
    /// The worker retired one command successfully.
    Progressed,
    /// The worker found no runnable command.
    Idle,
    /// The worker observed a stall with the supplied reason.
    Stalled(AsyncStallReason),
}

/// Reason the async runner stalled on its most recent step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncStallReason {
    /// A command could not begin because execution resources were unavailable.
    ExecutionResourcePressure,
    /// A command retired but could not publish its completion.
    CompletionPublicationPressure,
    /// A faulting command could not publish its event.
    EventPublicationPressure,
    /// A faulting command is waiting to be retired after a fault event.
    FaultRetirementPendingRetry,
}

/// One decoded DMA result cached before result-ring publication is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDmaResult {
    /// Successful execution waiting for completion-ring publication.
    Completion(pdma_model_contract::semantic::CompletionRecord),
    /// Canonical submission fault waiting for event-ring publication.
    Event(pdma_model_contract::bridge::AbiEvent),
}
