use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

const PAGE_SIZE: u64 = 256;

#[derive(Clone, Debug, Default)]
pub(in crate::harness) struct PageTable(Arc<Mutex<BTreeSet<u64>>>);

impl PageTable {
    pub(in crate::harness) fn evict(&self, address: MemoryAddress) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(address.raw() / PAGE_SIZE);
    }

    pub(in crate::harness) fn repair(&self, address: MemoryAddress) -> bool {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&(address.raw() / PAGE_SIZE))
    }
}

#[derive(Debug)]
pub(super) struct Paged {
    pub memory: SharedMemoryAccess,
    pub pages: PageTable,
}

impl Paged {
    fn access<T>(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
        action: impl FnOnce() -> Result<T, MemoryAccessError>,
    ) -> Result<T, MemoryAccessError> {
        self.memory.validate(address, alignment, length)?;
        let missing = self
            .pages
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if length != 0
            && let Some(page) = missing
                .range(address.raw() / PAGE_SIZE..=(address.raw() + length - 1) / PAGE_SIZE)
                .next()
        {
            return Err(MemoryAccessError::PageFault {
                address: (page * PAGE_SIZE).max(address.raw()).into(),
            });
        }
        let result = action();
        drop(missing);
        result
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
        self.access(address, alignment, length, || Ok(()))
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        out: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.access(
            address,
            alignment,
            u64::try_from(out.len()).map_err(|_| MemoryAccessError::AddressOverflow {
                address,
                length: u64::MAX,
            })?,
            || self.memory.read(address, alignment, out),
        )
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.access(
            address,
            alignment,
            u64::try_from(bytes.len()).map_err(|_| MemoryAccessError::AddressOverflow {
                address,
                length: u64::MAX,
            })?,
            || self.memory.write(address, alignment, bytes),
        )
    }
}
