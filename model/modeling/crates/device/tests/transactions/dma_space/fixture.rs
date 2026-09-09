#![allow(clippy::expect_used, reason = "checked test fixture operations")]

use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory, SharedMemoryAllocator,
};
use std::sync::Arc;

pub(super) fn fixture() -> (Arc<SharedMemory>, SharedMemoryAllocator) {
    let memory = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&memory), MemoryAddress::new(0x1_0000));
    (memory, allocator)
}

pub(super) fn alignment() -> MemoryAlignment {
    MemoryAlignment::try_new(64).expect("alignment")
}

pub(super) fn region(base: u64, length: u64) -> MemoryRegion {
    MemoryRegion::try_new(MemoryAddress::new(base), length, alignment()).expect("region")
}

pub(super) fn read(memory: &SharedMemory, address: MemoryAddress, length: usize) -> Vec<u8> {
    let mut bytes = vec![0; length];
    memory
        .read(
            address,
            MemoryAlignment::try_new(1).expect("byte alignment"),
            &mut bytes,
        )
        .expect("read");
    bytes
}
