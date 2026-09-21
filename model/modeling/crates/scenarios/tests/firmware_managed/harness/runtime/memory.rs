use hardware_interface_runtime::{MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{ChannelId, FunctionId};
use pdma_model_hw_components::ChannelWindows;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(super) struct FunctionSpaces<M>(Vec<M>);

impl<M> FunctionSpaces<M> {
    const fn new(spaces: Vec<M>) -> Self {
        Self(spaces)
    }

    fn get(&self, function: FunctionId, address: MemoryAddress) -> Result<&M, MemoryAccessError> {
        usize::try_from(function.raw())
            .ok()
            .and_then(|index| self.0.get(index))
            .ok_or(MemoryAccessError::Unmapped { address })
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
        self.get(*function, address)?
            .validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        function: &FunctionId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.get(*function, address)?
            .read(address, alignment, bytes)
    }

    fn write_with_sideband(
        &self,
        function: &FunctionId,
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
}

impl<M> RoutedMemory<M> {
    pub(super) const fn new(windows: Arc<Mutex<ChannelWindows>>, spaces: Vec<M>) -> Self {
        Self {
            windows,
            spaces: FunctionSpaces::new(spaces),
        }
    }
}

impl<M: MemoryAccess> MemoryAccess<ChannelId> for RoutedMemory<M> {
    fn validate_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .validate_with_sideband(channel, address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .read_with_sideband(channel, address, alignment, bytes)
    }

    fn write_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .write_with_sideband(channel, address, alignment, bytes)
    }
}
