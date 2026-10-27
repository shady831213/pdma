use hardware_interface_memory::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
};
use std::cell::Cell;

#[derive(Debug, Default)]
pub(super) struct Observed {
    pub(super) memory: SharedMemory,
    pub(super) interactions: Cell<usize>,
    pub(super) source_list_reads: Cell<usize>,
    pub(super) destination_list_reads: Cell<usize>,
    pub(super) payload_reads: Cell<usize>,
    pub(super) payload_writes: Cell<usize>,
}

impl MemoryAccess for Observed {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.interactions.set(self.interactions.get() + 1);
        self.memory.validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.interactions.set(self.interactions.get() + 1);
        if address == MemoryAddress::new(0x1800) {
            self.source_list_reads.set(self.source_list_reads.get() + 1);
        }
        if address == MemoryAddress::new(0x1900) {
            self.destination_list_reads
                .set(self.destination_list_reads.get() + 1);
        }
        if [MemoryAddress::new(0x1200), MemoryAddress::new(0x1300)].contains(&address) {
            self.payload_reads.set(self.payload_reads.get() + 1);
        }
        self.memory.read(address, alignment, destination)
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.interactions.set(self.interactions.get() + 1);
        if [MemoryAddress::new(0x1400), MemoryAddress::new(0x1500)].contains(&address) {
            self.payload_writes.set(self.payload_writes.get() + 1);
        }
        self.memory.write(address, alignment, source)
    }
}
