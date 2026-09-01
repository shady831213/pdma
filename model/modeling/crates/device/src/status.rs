/// Reason the device cannot advance its current deterministic step.
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
