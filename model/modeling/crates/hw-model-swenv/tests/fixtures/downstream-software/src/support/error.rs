use hir::EnvContextError;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    sync::{Mutex, MutexGuard},
};

#[derive(Debug)]
pub(super) enum ConsumerError {
    Env(EnvContextError),
    Gate(hw_model_io::register::observation::ObservationGateError),
    RegisterShape,
    Invariant(&'static str),
}

impl Display for ConsumerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Env(error) => Display::fmt(error, formatter),
            Self::Gate(error) => Display::fmt(error, formatter),
            Self::RegisterShape => formatter.write_str("consumer register shape rejected"),
            Self::Invariant(message) => formatter.write_str(message),
        }
    }
}

impl Error for ConsumerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Env(error) => Some(error),
            Self::Gate(error) => Some(error),
            Self::RegisterShape | Self::Invariant(_) => None,
        }
    }
}

impl From<EnvContextError> for ConsumerError {
    fn from(error: EnvContextError) -> Self {
        Self::Env(error)
    }
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct EnvCounts {
    pub(super) main_segments: usize,
    pub(super) handler_entries: usize,
}
