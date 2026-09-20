use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
};
use std::cell::Cell;

#[derive(Debug)]
pub(super) struct Observed {
    pub(super) memory: SharedMemory,
    pub(super) payload_read: Cell<bool>,
}

impl MemoryAccess for Observed {
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.memory.validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.read(address, alignment, destination)?;
        if address == MemoryAddress::new(0x1200) {
            self.payload_read.set(true);
        }
        Ok(())
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.write(address, alignment, source)
    }
}
