use super::PlatformConfigError;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::DmaAccess;
use pdma_model_dma::MemoryFailure;
use pdma_model_hw_components::{context, windows};
use std::{
    fmt,
    sync::{Arc, Mutex},
};

mod control;
mod spaces;
pub(super) use control::ControlMemory;
pub(super) use spaces::FunctionSpaces;
pub(in crate::harness) use spaces::SpaceError;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RouteError<E> {
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

#[derive(Debug)]
pub(super) struct RoutedMemory<M> {
    windows: Arc<Mutex<windows::ChannelWindows>>,
    spaces: FunctionSpaces<M>,
    contexts: Option<Arc<Mutex<context::ContextTable>>>,
}

impl<M> RoutedMemory<M> {
    pub(super) fn new(
        windows: Arc<Mutex<windows::ChannelWindows>>,
        spaces: Vec<M>,
    ) -> Result<Self, PlatformConfigError> {
        Ok(Self {
            windows,
            spaces: FunctionSpaces::new(spaces)?,
            contexts: None,
        })
    }

    pub(super) const fn with_contexts(
        windows: Arc<Mutex<windows::ChannelWindows>>,
        contexts: Arc<Mutex<context::ContextTable>>,
        spaces: FunctionSpaces<M>,
    ) -> Self {
        Self {
            windows,
            contexts: Some(contexts),
            spaces,
        }
    }
}

impl<M: MemoryAccess> MemoryAccess<DmaAccess> for RoutedMemory<M> {
    type Error = RouteError<M::Error>;

    fn validate_with_sideband(
        &self,
        channel: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.contexts.as_ref().map_or_else(
            || {
                windows
                    .memory(&self.spaces)
                    .validate_with_sideband(&channel.channel, address, alignment, length)
                    .map_err(RouteError::Window)
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .validate_with_sideband(channel, address, alignment, length)
                    .map_err(RouteError::Context)
            },
        )
    }

    fn read_with_sideband(
        &self,
        channel: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &self.contexts {
            Some(contexts) => contexts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .memory(&windows, &self.spaces)
                .read_with_sideband(channel, address, alignment, bytes)
                .map_err(RouteError::Context),
            None => windows
                .memory(&self.spaces)
                .read_with_sideband(&channel.channel, address, alignment, bytes)
                .map_err(RouteError::Window),
        }
    }

    fn write_with_sideband(
        &self,
        channel: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.contexts.as_ref().map_or_else(
            || {
                windows
                    .memory(&self.spaces)
                    .write_with_sideband(&channel.channel, address, alignment, bytes)
                    .map_err(RouteError::Window)
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .write_with_sideband(channel, address, alignment, bytes)
                    .map_err(RouteError::Context)
            },
        )
    }
}
