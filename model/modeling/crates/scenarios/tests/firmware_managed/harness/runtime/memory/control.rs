use super::{FunctionSpaces, SpaceError};
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAddress, MemoryAlignment,
};
use pdma_model_hw_components::control::ControlChannel;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(in crate::harness::runtime) struct ControlMemory<M> {
    pub(in crate::harness::runtime) control: Arc<Mutex<ControlChannel>>,
    pub(in crate::harness::runtime) spaces: FunctionSpaces<M>,
}

impl<M: MemoryAccess> MemoryAccess for ControlMemory<M> {
    type Error = pdma_model_hw_components::control::MemoryError<SpaceError<M::Error>>;
    fn atomic32_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .atomic32(address, operation)
    }

    fn atomic64_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .atomic64(address, operation)
    }

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .memory(&self.spaces)
            .write(address, alignment, bytes)
    }
}
