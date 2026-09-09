use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAllocationError, MemoryError,
    SharedMemory,
};
mod fixture;
use fixture::{alignment, fixture, read, region};

#[test]
fn prepared_batch_installs_all_regions_together() {
    // Given: one store and three runtime regions.
    let memory = SharedMemory::new();
    let regions = [
        region(0x1_0000, 3),
        region(0x1_0040, 1),
        region(0x1_0080, 65),
    ];
    let prepared = memory.prepare_regions(&regions).expect("prepare");
    assert!(memory.regions().is_empty());
    // When: the prepared regions are installed.
    prepared.try_install(&memory).expect("install");
    // Then: all exact extents are visible after one revision change.
    assert_eq!(memory.regions(), regions);
    assert_eq!(memory.revision().raw(), 1);
    for entry in regions {
        let length = usize::try_from(entry.length()).expect("length");
        assert_eq!(read(&memory, entry.base(), length), vec![0; length]);
    }
}

#[test]
fn allocation_exposes_aligned_address_and_zeroed_backing() {
    // Given: a runtime-owned allocator and store.
    let (memory, allocator) = fixture();
    // When: one seven-byte allocation commits.
    let address = allocator.allocate(7, alignment()).expect("allocate");
    // Then: exact backing metadata and bytes belong to the runtime.
    assert_eq!(address, MemoryAddress::new(0x1_0000));
    assert_eq!(memory.regions(), vec![region(0x1_0000, 7)]);
    assert_eq!(read(&memory, address, 7), vec![0; 7]);
    assert_eq!(memory.revision().raw(), 1);
}

#[test]
fn allocation_exhaustion_preserves_allocator_state() {
    // Given: a committed extent with sentinel bytes.
    let (memory, allocator) = fixture();
    let first = allocator.allocate(4, alignment()).expect("first");
    memory
        .write(first, alignment(), &[1, 2, 3, 4])
        .expect("sentinel");
    let revision = memory.revision();
    // When: a request overflows the remaining address space.
    let exhausted = allocator.allocate(u64::MAX, alignment());
    // Then: no cursor advancement or store mutation precedes the next success.
    assert_eq!(exhausted, Err(MemoryAllocationError::AddressExhausted));
    assert_eq!(memory.revision(), revision);
    assert_eq!(memory.regions(), vec![region(0x1_0000, 4)]);
    assert_eq!(read(&memory, first, 4), vec![1, 2, 3, 4]);
    let next = allocator.allocate(4, alignment()).expect("next");
    assert_eq!(next, MemoryAddress::new(0x1_0040));
    assert_eq!(read(&memory, next, 4), vec![0; 4]);
    assert_eq!(memory.revision().raw(), revision.raw() + 1);
}

#[test]
fn dropped_and_conflicting_proposals_preserve_regions_and_bytes() {
    // Given: sentinel data and three proposals from the same revision.
    let (memory, allocator) = fixture();
    let initial = allocator.allocate(4, alignment()).expect("initial");
    memory
        .write(initial, alignment(), &[1, 2, 3, 4])
        .expect("sentinel");
    let proposed = [region(0x1_0040, 8)];
    let dropped = memory.prepare_regions(&proposed).expect("drop proposal");
    let winner = memory.prepare_regions(&proposed).expect("winner");
    let conflicting = memory.prepare_regions(&proposed).expect("conflict");
    // When: one proposal drops, another commits, and the stale proposal attempts install.
    drop(dropped);
    winner.try_install(&memory).expect("commit");
    let stale = conflicting.try_install(&memory);
    // Then: only the winning proposal changes the installed region set.
    assert_eq!(
        stale,
        Err(MemoryError::StaleRevision {
            expected_revision: 1,
            actual_revision: 2
        })
    );
    assert_eq!(memory.revision().raw(), 2);
    assert_eq!(memory.regions(), vec![region(0x1_0000, 4), proposed[0]]);
    assert_eq!(read(&memory, initial, 4), vec![1, 2, 3, 4]);
}

#[test]
fn overlap_and_cross_region_access_preserve_existing_bytes() {
    // Given: adjacent runtime allocations with sentinel data.
    let (memory, allocator) = fixture();
    let first = allocator.allocate(64, alignment()).expect("first");
    let second = allocator.allocate(64, alignment()).expect("second");
    memory.write(first, alignment(), &[0xaa]).expect("sentinel");
    let proposed = region(second.raw(), 128);
    // When: an overlapping proposal and two invalid accesses are attempted.
    let overlap = memory.prepare_regions(&[proposed]);
    let unmapped = memory.write(MemoryAddress::new(0), alignment(), &[1]);
    let crossing = memory.write(first, alignment(), &[0xbb; 65]);
    // Then: each error leaves both existing allocations unchanged.
    assert_eq!(
        overlap,
        Err(MemoryError::Overlap {
            first: region(second.raw(), 64),
            second: proposed
        })
    );
    assert_eq!(
        unmapped,
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0)
        })
    );
    assert_eq!(
        crossing,
        Err(MemoryAccessError::CrossesRegion {
            address: first,
            length: 65,
            region: region(first.raw(), 64)
        })
    );
    assert_eq!(memory.revision().raw(), 2);
    assert_eq!(read(&memory, first, 1), vec![0xaa]);
    assert_eq!(read(&memory, second, 1), vec![0]);
}

#[test]
fn zero_length_allocation_reserves_address_without_backing() {
    // Given: an empty runtime allocator.
    let (memory, allocator) = fixture();
    // When: an empty allocation precedes a nonempty allocation.
    let empty = allocator.allocate(0, alignment()).expect("empty");
    let following = allocator.allocate(1, alignment()).expect("following");
    // Then: only the nonempty allocation is backed, with the empty address reserved.
    assert_eq!(empty, MemoryAddress::new(0x1_0000));
    assert_eq!(following, MemoryAddress::new(0x1_0040));
    assert_eq!(memory.regions(), vec![region(following.raw(), 1)]);
    assert_eq!(
        memory.write(empty, alignment(), &[]),
        Err(MemoryAccessError::Unmapped { address: empty })
    );
    assert_eq!(read(&memory, following, 1), vec![0]);
}
