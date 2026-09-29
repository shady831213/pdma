use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
};
use std::cell::Cell;

#[derive(Debug)]
pub(super) struct Paged {
    pub memory: SharedMemory,
    pub missing: Cell<Option<(u64, bool)>>,
    pub faults: Cell<u32>,
}

impl Paged {
    fn fault(
        &self,
        address: MemoryAddress,
        length: usize,
        write: bool,
    ) -> Result<(), MemoryAccessError> {
        if let Some((missing, writes)) = self.missing.get()
            && writes == write
            && missing >= address.raw()
            && missing - address.raw()
                < u64::try_from(length).map_err(|_| MemoryAccessError::AddressOverflow {
                    address,
                    length: u64::MAX,
                })?
        {
            self.faults.set(self.faults.get() + 1);
            return Err(MemoryAccessError::PageFault {
                address: missing.into(),
            });
        }
        Ok(())
    }
}

impl MemoryAccess for Paged {
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
        out: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.fault(address, out.len(), false)?;
        self.memory.read(address, alignment, out)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.fault(address, bytes.len(), true)?;
        self.memory.write(address, alignment, bytes)
    }
}
