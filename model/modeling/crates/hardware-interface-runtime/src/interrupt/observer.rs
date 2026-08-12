use super::traits::InterruptEnvelope;
use std::{error::Error, sync::Arc};

/// One synchronous handler lifecycle observation.
#[derive(Debug)]
pub enum InterruptLifecycleEvent<'a, N>
where
    N: InterruptEnvelope,
{
    /// Immediately before the handler's first poll.
    Started {
        /// Accepted notification.
        notification: &'a N,
        /// Resolved handler vector.
        vector: &'a N::Vector,
    },
    /// Successful handler completion before CPU release.
    Succeeded {
        /// Accepted notification.
        notification: &'a N,
        /// Resolved handler vector.
        vector: &'a N::Vector,
    },
    /// Failed handler completion before CPU release.
    Failed {
        /// Accepted notification.
        notification: &'a N,
        /// Resolved handler vector.
        vector: &'a N::Vector,
        /// Original typed handler failure.
        error: &'a (dyn Error + Send + Sync),
    },
}

/// Object-safe, synchronous and infallible lifecycle observer.
pub trait InterruptLifecycleObserver<N>: Send + Sync
where
    N: InterruptEnvelope,
{
    /// Records one lifecycle event without changing dispatcher outcome.
    fn observe(&self, event: InterruptLifecycleEvent<'_, N>);
}

/// Clone-shared lifecycle observer handle.
pub type SharedInterruptObserver<N> = Arc<dyn InterruptLifecycleObserver<N>>;

/// Observer that intentionally records nothing.
#[derive(Debug, Default)]
pub struct NoopInterruptObserver;

impl<N> InterruptLifecycleObserver<N> for NoopInterruptObserver
where
    N: InterruptEnvelope,
{
    fn observe(&self, _event: InterruptLifecycleEvent<'_, N>) {}
}
