use super::SpaceError;
use pdma_model_dma::MemoryFailure;
use pdma_model_hw_components::{context, windows};
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub(in crate::harness::runtime) enum RouteError<E> {
    Window(windows::MemoryError<SpaceError<E>>),
    Context(context::MemoryError<SpaceError<E>>),
}

impl<E: fmt::Display> fmt::Display for RouteError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(error) => error.fmt(f),
            Self::Context(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RouteError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Window(error) => Some(error),
            Self::Context(error) => Some(error),
        }
    }
}

impl<E: Into<Self>> From<RouteError<E>> for MemoryFailure {
    fn from(error: RouteError<E>) -> Self {
        match error {
            RouteError::Window(error) => error.into(),
            RouteError::Context(error) => error.into(),
        }
    }
}
