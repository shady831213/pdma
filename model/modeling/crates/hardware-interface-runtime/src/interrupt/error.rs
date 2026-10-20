use std::{error::Error, fmt};

/// Terminal dispatcher failure preserving concrete collaborator errors.
#[derive(Debug)]
pub enum InterruptDispatchError<V, S, H> {
    /// Readiness wait failed.
    Readiness(S),
    /// Claim failed.
    Claim(S),
    /// Claimed notification has no registered vector.
    UnregisteredVector(V),
    /// Persistent handler failed.
    Handler {
        /// Vector whose persistent handler failed.
        vector: V,
        /// Original concrete handler failure.
        source: H,
    },
}

impl<V: fmt::Debug, S, H> fmt::Display for InterruptDispatchError<V, S, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Readiness(_) => formatter.write_str("interrupt readiness failed"),
            Self::Claim(_) => formatter.write_str("interrupt claim failed"),
            Self::UnregisteredVector(vector) => {
                write!(formatter, "unregistered interrupt vector {vector:?}")
            }
            Self::Handler { vector, .. } => {
                write!(formatter, "interrupt handler failed for vector {vector:?}")
            }
        }
    }
}

impl<V, S, H> Error for InterruptDispatchError<V, S, H>
where
    V: fmt::Debug,
    S: Error + 'static,
    H: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Readiness(source) | Self::Claim(source) => Some(source),
            Self::Handler { source, .. } => Some(source),
            Self::UnregisteredVector(_) => None,
        }
    }
}

/// Failure observed while joining the sole dispatcher task.
#[derive(Debug)]
pub enum InterruptJoinError<V, S, H> {
    /// Dispatcher terminated with a typed operational failure.
    Dispatch(InterruptDispatchError<V, S, H>),
    /// Tokio task terminated unexpectedly.
    Task(tokio::task::JoinError),
}

impl<V: fmt::Debug, S, H> fmt::Display for InterruptJoinError<V, S, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dispatch(error) => error.fmt(formatter),
            Self::Task(_) => formatter.write_str("interrupt dispatcher task failed"),
        }
    }
}

impl<V, S, H> Error for InterruptJoinError<V, S, H>
where
    V: fmt::Debug + 'static,
    S: Error + 'static,
    H: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Dispatch(error) => Some(error),
            Self::Task(error) => Some(error),
        }
    }
}
