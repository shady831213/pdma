#![allow(missing_docs, reason = "external fixture test")]

use hardware_interface_io::memory::storage::{LocalStorage, MutexStorage, StorageBackend};
use hardware_interface_io::memory::{
    LocalMemoryPolicy, Memory, MemoryPolicy, MemoryRegion, SharedMemoryPolicy,
};
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};

fn storage_transcript<B: StorageBackend<Vec<u8>>>(
    storage: &B,
) -> Result<Vec<u8>, hardware_interface_io::memory::storage::StorageError> {
    storage.mutate(|bytes| bytes.extend_from_slice(&[0x22, 0x33]))?;
    storage.inspect(Clone::clone)
}

fn memory_transcript<P: MemoryPolicy>() -> Result<[u8; 4], Box<dyn std::error::Error>> {
    let memory = Memory::<P>::new();
    let region = MemoryRegion::try_new(
        MemoryAddress::new(0x4000),
        16,
        MemoryAlignment::try_new(16)?,
    )?;
    memory.add_regions(&[region])?;
    memory.write(
        MemoryAddress::new(0x4004),
        MemoryAlignment::try_new(4)?,
        &[0xde, 0xad, 0xbe, 0xef],
    )?;
    let mut bytes = [0_u8; 4];
    memory.read(
        MemoryAddress::new(0x4004),
        MemoryAlignment::try_new(4)?,
        &mut bytes,
    )?;
    Ok(bytes)
}

#[test]
fn local_and_shared_storage_and_memory_have_literal_parity()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: local and mutex storage plus local and shared generic memories.
    let local_storage = LocalStorage::new(vec![0x11]);
    let shared_storage = MutexStorage::new(vec![0x11]);

    // When: the same mutations and checked-memory accesses use each policy.
    let local_values = storage_transcript(&local_storage)?;
    let shared_values = storage_transcript(&shared_storage)?;
    let local_memory = memory_transcript::<LocalMemoryPolicy>()?;
    let shared_memory = memory_transcript::<SharedMemoryPolicy>()?;

    // Then: policy choice does not change literal values or memory bytes.
    assert_eq!(local_values, [0x11, 0x22, 0x33]);
    assert_eq!(shared_values, local_values);
    assert_eq!(local_memory, [0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(shared_memory, local_memory);
    println!(
        "backends storage_local=[11,22,33] storage_mutex=[11,22,33] memory_local=[de,ad,be,ef] memory_shared=[de,ad,be,ef]"
    );
    Ok(())
}
