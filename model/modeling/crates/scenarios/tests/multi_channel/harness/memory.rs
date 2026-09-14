use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use std::sync::Arc;
use tokio::sync::Notify;

pub(super) struct Observed {
    pub(super) memory: SharedMemoryAccess,
    pub(super) changed: Arc<Notify>,
}

impl MemoryAccess for Observed {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.memory.validate(address, alignment, length)
    }

    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.read(address, alignment, destination)
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.write(address, alignment, source)?;
        self.changed.notify_one();
        Ok(())
    }
}
