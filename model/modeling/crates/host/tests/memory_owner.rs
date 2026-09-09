//! Software memory ownership and provisioning regression tests.

use hardware_interface_runtime::{MemoryAccess, MemoryAlignment, SharedMemory};
use pdma_model_host::adapter::abi::{PdmaHostMemory, PdmaRegionRequest};
use std::sync::Arc;

#[test]
fn software_memory_owns_buffer_identity_without_a_device() {
    // Given: software shares one independently provisioned runtime store.
    let storage = Arc::new(SharedMemory::new());
    let memory = PdmaHostMemory::new(storage.clone());
    let peer = memory.clone();
    let alignment = MemoryAlignment::try_new(64).expect("alignment");

    // When: both owners allocate and exchange data through their shared regions.
    let first = memory
        .allocate_region(PdmaRegionRequest::new(4, alignment))
        .expect("first region");
    let second = peer
        .allocate_region(PdmaRegionRequest::new(4, alignment))
        .expect("second region");
    memory
        .write(first.base(), MemoryAlignment::ONE, &[1, 2, 3, 4])
        .expect("write");
    let mut bytes = [0; 4];
    peer.read(first.base(), MemoryAlignment::ONE, &mut bytes)
        .expect("read");

    // Then: identities and bytes are software-owned and visible to both clones.
    assert_ne!(first.object(), second.object());
    assert_eq!(second.object().raw(), first.object().raw() + 1);
    assert_eq!(bytes, [1, 2, 3, 4]);
    assert_eq!(storage.regions().len(), 2);
}

#[test]
fn exhausted_allocation_preserves_software_identity_and_runtime_bytes() {
    // Given: one successfully allocated software buffer.
    let storage = Arc::new(SharedMemory::new());
    let memory = PdmaHostMemory::new(storage.clone());
    let alignment = MemoryAlignment::try_new(64).expect("alignment");
    let first = memory
        .allocate_region(PdmaRegionRequest::new(4, alignment))
        .expect("first region");
    memory
        .write(first.base(), alignment, &[9, 8, 7, 6])
        .expect("sentinel bytes");
    let revision = storage.revision();

    // When: a request exhausts the address space.
    let failed = memory.allocate_region(PdmaRegionRequest::new(u64::MAX, alignment));

    // Then: no runtime allocation or software identity was committed.
    assert!(failed.is_err());
    assert_eq!(storage.revision(), revision);
    let mut bytes = [0; 4];
    memory
        .read(first.base(), alignment, &mut bytes)
        .expect("preserved bytes");
    assert_eq!(bytes, [9, 8, 7, 6]);
    let next = memory
        .allocate_region(PdmaRegionRequest::new(4, alignment))
        .expect("next region");
    assert_eq!(next.object().raw(), first.object().raw() + 1);
}

#[test]
fn empty_buffers_have_distinct_identities_without_runtime_storage() {
    // Given: shared software memory with no installed runtime regions.
    let storage = Arc::new(SharedMemory::new());
    let memory = PdmaHostMemory::new(storage.clone());
    let alignment = MemoryAlignment::try_new(64).expect("alignment");
    let revision = storage.revision();

    // When: software creates two zero-length logical buffers.
    let first = memory
        .allocate_region(PdmaRegionRequest::new(0, alignment))
        .expect("first empty");
    let second = memory
        .allocate_region(PdmaRegionRequest::new(0, alignment))
        .expect("second empty");

    // Then: logical accesses succeed without creating storage or aliasing identities.
    assert_ne!(first.object(), second.object());
    assert_ne!(first.base(), second.base());
    assert_eq!(first.storage(), None);
    assert!(storage.regions().is_empty());
    assert_eq!(storage.revision(), revision);
    assert_eq!(memory.read(first.base(), alignment, &mut []), Ok(()));
    assert_eq!(memory.write(second.base(), alignment, &[]), Ok(()));
    assert_eq!(memory.validate(first.base(), alignment, 0), Ok(()));
    assert!(memory.validate(first.base(), alignment, 1).is_err());
    assert!(memory.read(first.base(), alignment, &mut [0]).is_err());
}
