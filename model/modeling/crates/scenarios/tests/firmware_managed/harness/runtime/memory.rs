use super::PlatformConfigError;
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::DmaAccess;
use pdma_model_contract::memory::{ContextAttributes, FunctionId, MemoryIdentity};
use pdma_model_hw_components::{context::ContextTable, windows::ChannelWindows};
use std::sync::{Arc, Mutex};

mod control;
pub(super) use control::ControlMemory;

#[derive(Debug)]
pub(super) struct FunctionSpaces<M>(pub(super) Vec<(MemoryIdentity, M)>);

impl<M> FunctionSpaces<M> {
    pub(super) fn new(spaces: Vec<M>) -> Result<Self, PlatformConfigError> {
        Ok(Self(
            spaces
                .into_iter()
                .enumerate()
                .map(|(index, memory)| {
                    Ok((
                        MemoryIdentity {
                            function: FunctionId::new(
                                u32::try_from(index)
                                    .map_err(|_| PlatformConfigError::FunctionCount)?,
                            ),
                            attributes: ContextAttributes::DEFAULT,
                        },
                        memory,
                    ))
                })
                .collect::<Result<_, PlatformConfigError>>()?,
        ))
    }

    fn get(
        &self,
        identity: MemoryIdentity,
        address: MemoryAddress,
    ) -> Result<&M, MemoryAccessError> {
        self.0
            .iter()
            .find(|(key, _)| *key == identity)
            .map(|(_, memory)| memory)
            .ok_or(MemoryAccessError::Unmapped { address })
    }
}

impl<M: MemoryAccess> MemoryAccess<MemoryIdentity> for FunctionSpaces<M> {
    fn validate_with_sideband(
        &self,
        function: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.get(*function, address)?
            .validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        function: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(*function, address)?
            .read(address, alignment, bytes)
    }

    fn write_with_sideband(
        &self,
        function: &MemoryIdentity,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(*function, address)?
            .write(address, alignment, bytes)
    }
}

#[derive(Debug)]
pub(super) struct RoutedMemory<M> {
    windows: Arc<Mutex<ChannelWindows>>,
    spaces: FunctionSpaces<M>,
    contexts: Option<Arc<Mutex<ContextTable>>>,
}

impl<M> RoutedMemory<M> {
    pub(super) fn new(
        windows: Arc<Mutex<ChannelWindows>>,
        spaces: Vec<M>,
    ) -> Result<Self, PlatformConfigError> {
        Ok(Self {
            windows,
            spaces: FunctionSpaces::new(spaces)?,
            contexts: None,
        })
    }

    pub(super) const fn with_contexts(
        windows: Arc<Mutex<ChannelWindows>>,
        contexts: Arc<Mutex<ContextTable>>,
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
    fn validate_with_sideband(
        &self,
        channel: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.contexts.as_ref().map_or_else(
            || {
                windows.memory(&self.spaces).validate_with_sideband(
                    &channel.channel,
                    address,
                    alignment,
                    length,
                )
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .validate_with_sideband(channel, address, alignment, length)
            },
        )
    }

    fn read_with_sideband(
        &self,
        channel: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &self.contexts {
            Some(contexts) => contexts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .memory(&windows, &self.spaces)
                .read_with_sideband(channel, address, alignment, bytes),
            None => windows.memory(&self.spaces).read_with_sideband(
                &channel.channel,
                address,
                alignment,
                bytes,
            ),
        }
    }

    fn write_with_sideband(
        &self,
        channel: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.contexts.as_ref().map_or_else(
            || {
                windows.memory(&self.spaces).write_with_sideband(
                    &channel.channel,
                    address,
                    alignment,
                    bytes,
                )
            },
            |contexts| {
                contexts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .memory(&windows, &self.spaces)
                    .write_with_sideband(channel, address, alignment, bytes)
            },
        )
    }
}

impl<M: MemoryAccess> MemoryAccess<FunctionId> for FunctionSpaces<M> {
    fn validate_with_sideband(
        &self,
        function: &FunctionId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.validate_with_sideband(
            &MemoryIdentity {
                function: *function,
                attributes: ContextAttributes::DEFAULT,
            },
            address,
            alignment,
            length,
        )
    }
    fn read_with_sideband(
        &self,
        function: &FunctionId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.read_with_sideband(
            &MemoryIdentity {
                function: *function,
                attributes: ContextAttributes::DEFAULT,
            },
            address,
            alignment,
            bytes,
        )
    }
    fn write_with_sideband(
        &self,
        function: &FunctionId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.write_with_sideband(
            &MemoryIdentity {
                function: *function,
                attributes: ContextAttributes::DEFAULT,
            },
            address,
            alignment,
            bytes,
        )
    }
}
