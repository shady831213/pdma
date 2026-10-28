use super::PlatformConfigError;
use super::platform::MemoryRequest;
use hw_model_io::memory::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAddress, MemoryAlignment,
};
use pdma_model_contract::dma::{ChannelId, DmaAccess};
use pdma_model_contract::memory::{ContextAttributes, FunctionId, MemoryIdentity};
use pdma_model_hw_components::{context, windows};
use std::sync::{Arc, Mutex};

mod control;
mod error;
mod spaces;
pub(super) use control::ControlMemory;
pub(super) use error::RouteError;
pub(super) use spaces::FunctionSpaces;
pub(in crate::harness) use spaces::SpaceError;

const fn window_request(channel: ChannelId, function: FunctionId) -> MemoryRequest {
    MemoryRequest {
        channel: Some(channel),
        identity: MemoryIdentity {
            function,
            attributes: ContextAttributes::DEFAULT,
        },
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

    fn atomic32_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.contexts.as_ref().map_or_else(
            || {
                windows
                    .memory(&self.spaces)
                    .map_sideband(window_request)
                    .atomic32_with_sideband(&access.channel, address, operation)
                    .map_err(|error| match error {
                        AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                        AtomicAccessError::Access(error) => {
                            AtomicAccessError::Access(RouteError::Window(error))
                        }
                    })
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .map_sideband(|access, identity| MemoryRequest {
                        channel: Some(access.channel),
                        identity,
                    })
                    .atomic32_with_sideband(access, address, operation)
                    .map_err(|error| match error {
                        AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                        AtomicAccessError::Access(error) => {
                            AtomicAccessError::Access(RouteError::Context(error))
                        }
                    })
            },
        )
    }

    fn atomic64_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.contexts.as_ref().map_or_else(
            || {
                windows
                    .memory(&self.spaces)
                    .map_sideband(window_request)
                    .atomic64_with_sideband(&access.channel, address, operation)
                    .map_err(|error| match error {
                        AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                        AtomicAccessError::Access(error) => {
                            AtomicAccessError::Access(RouteError::Window(error))
                        }
                    })
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .map_sideband(|access, identity| MemoryRequest {
                        channel: Some(access.channel),
                        identity,
                    })
                    .atomic64_with_sideband(access, address, operation)
                    .map_err(|error| match error {
                        AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                        AtomicAccessError::Access(error) => {
                            AtomicAccessError::Access(RouteError::Context(error))
                        }
                    })
            },
        )
    }

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
                    .map_sideband(window_request)
                    .validate_with_sideband(&channel.channel, address, alignment, length)
                    .map_err(RouteError::Window)
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .map_sideband(|access, identity| MemoryRequest {
                        channel: Some(access.channel),
                        identity,
                    })
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
                .map_sideband(|access, identity| MemoryRequest {
                    channel: Some(access.channel),
                    identity,
                })
                .read_with_sideband(channel, address, alignment, bytes)
                .map_err(RouteError::Context),
            None => windows
                .memory(&self.spaces)
                .map_sideband(window_request)
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
                    .map_sideband(window_request)
                    .write_with_sideband(&channel.channel, address, alignment, bytes)
                    .map_err(RouteError::Window)
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .map_sideband(|access, identity| MemoryRequest {
                        channel: Some(access.channel),
                        identity,
                    })
                    .write_with_sideband(channel, address, alignment, bytes)
                    .map_err(RouteError::Context)
            },
        )
    }
}
