use std::{error::Error, future::Future};

/// A source-owned notification carrying a registry lookup key.
pub trait InterruptEnvelope: Send + Sync + 'static {
    /// Registry key selecting one persistent handler.
    type Vector: Clone + Ord + Send + Sync + 'static;

    /// Returns this notification's handler key.
    fn vector(&self) -> Self::Vector;
}

/// A cancellation-safe two-phase interrupt notification source.
pub trait InterruptSource: Send + 'static {
    /// Claimed notification type.
    type Notification: InterruptEnvelope;
    /// Affine readiness capability passed back to `claim`.
    type Ready: Send;
    /// Typed source failure.
    type Error: Error + Send + Sync + 'static;

    /// Waits non-consumingly for readiness.
    fn wait_ready(&self) -> impl Future<Output = Result<Self::Ready, Self::Error>> + Send;

    /// Serially claims at most one notification.
    fn claim(
        &mut self,
        ready: Self::Ready,
    ) -> impl Future<Output = Result<Option<Self::Notification>, Self::Error>> + Send;
}

/// Persistent mutable handler state for one notification type.
pub trait InterruptHandler<N>: Send {
    /// Typed handler failure.
    type Error: Error + Send + Sync + 'static;

    /// Handles one accepted notification while borrowing persistent state.
    fn handle(&mut self, notification: &N) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
