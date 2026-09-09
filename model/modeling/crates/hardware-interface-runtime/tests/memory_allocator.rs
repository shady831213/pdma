#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use std::sync::Arc;

use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryError, SharedMemory,
};
use support::{alignment, install, region};

#[test]
fn allocator_installs_aligned_zeroed_backing() {
    // Given a deliberately unaligned starting cursor.
    let memory = Arc::new(SharedMemory::new());
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        Arc::clone(&memory),
        MemoryAddress::new(0x1003),
    );

    // When a 64-byte region with 64-byte alignment is allocated.
    let address = allocator.allocate(64, alignment(64)).expect("allocate");

    // Then its public access capability exposes aligned, zeroed backing.
    assert_eq!(address, MemoryAddress::new(0x1040));
    assert_eq!(memory.regions(), [region(0x1040, 64, 64)]);
    let access = allocator.memory_access();
    assert!(access.shares_backend(&allocator.memory_access()));
    let mut bytes = [0xff; 64];
    access
        .read(address, alignment(64), &mut bytes)
        .expect("read allocation");
    assert_eq!(bytes, [0; 64]);
}

#[test]
fn allocator_clones_reserve_distinct_addresses_concurrently() {
    // Given two handles sharing one allocator cursor.
    let memory = Arc::new(SharedMemory::new());
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        Arc::clone(&memory),
        MemoryAddress::new(0x1000),
    );
    let other = allocator.clone();

    // When both handles allocate concurrently and are joined before observation.
    let mut addresses = std::thread::scope(|scope| {
        let first = scope.spawn(|| allocator.allocate(64, alignment(64)));
        let second = scope.spawn(|| other.allocate(64, alignment(64)));
        [
            first
                .join()
                .expect("first joined")
                .expect("first allocation"),
            second
                .join()
                .expect("second joined")
                .expect("second allocation"),
        ]
    });

    // Then each receives a unique consecutive region.
    addresses.sort_unstable();
    assert_eq!(
        addresses,
        [MemoryAddress::new(0x1000), MemoryAddress::new(0x1040)]
    );
    assert_eq!(
        memory.regions(),
        [region(0x1000, 64, 64), region(0x1040, 64, 64)]
    );
}

#[test]
fn zero_allocation_advances_cursor_without_installing_backing() {
    // Given empty storage with no available region capacity.
    let memory = Arc::new(SharedMemory::with_region_capacity(0));
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        Arc::clone(&memory),
        MemoryAddress::new(0x1000),
    );
    let before = memory.revision();

    // When two zero-length logical addresses are reserved.
    let first = allocator.allocate(0, alignment(1)).expect("first zero");
    let second = allocator.allocate(0, alignment(1)).expect("second zero");

    // Then addresses advance by one byte without creating storage or a valid extent.
    assert_eq!(first, MemoryAddress::new(0x1000));
    assert_eq!(second, MemoryAddress::new(0x1001));
    assert_eq!(memory.revision(), before);
    assert!(memory.regions().is_empty());
    assert_eq!(
        allocator.memory_access().validate(first, alignment(1), 0),
        Err(MemoryAccessError::Unmapped { address: first })
    );
}

#[test]
fn failed_allocation_preserves_cursor_and_installed_bytes() {
    // Given an installed sentinel region beyond the allocator's initial cursor.
    let memory = Arc::new(SharedMemory::new());
    install(&memory, &[region(0x1040, 64, 64)]);
    memory
        .write(MemoryAddress::new(0x1040), alignment(64), &[0xa5; 64])
        .expect("sentinel");
    let before = memory.revision();
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        Arc::clone(&memory),
        MemoryAddress::new(0x1000),
    );

    // When an allocation would overlap the installed sentinel.
    let result = allocator.allocate(128, alignment(64));

    // Then failure leaves storage and cursor untouched, allowing the smaller allocation.
    assert_eq!(
        result,
        Err(hardware_interface_runtime::MemoryAllocationError::Storage(
            MemoryError::Overlap {
                first: region(0x1040, 64, 64),
                second: region(0x1000, 128, 64)
            }
        ))
    );
    assert_eq!(memory.revision(), before);
    assert_eq!(memory.regions(), [region(0x1040, 64, 64)]);
    let mut bytes = [0; 64];
    memory
        .read(MemoryAddress::new(0x1040), alignment(64), &mut bytes)
        .expect("read sentinel");
    assert_eq!(bytes, [0xa5; 64]);
    assert_eq!(
        allocator.allocate(64, alignment(64)).expect("retry"),
        MemoryAddress::new(0x1000)
    );
}

#[test]
fn capacity_failure_preserves_cursor_for_zero_allocation() {
    // Given an allocator whose store rejects every non-empty region.
    let memory = Arc::new(SharedMemory::with_region_capacity(0));
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        Arc::clone(&memory),
        MemoryAddress::new(0x1003),
    );

    // When aligned backing installation fails.
    let result = allocator.allocate(64, alignment(64));

    // Then even alignment padding is not committed to the cursor.
    assert_eq!(
        result,
        Err(hardware_interface_runtime::MemoryAllocationError::Storage(
            MemoryError::RegionTableExhausted { entries: 1 }
        ))
    );
    assert_eq!(memory.revision().raw(), 0);
    assert!(memory.regions().is_empty());
    assert_eq!(
        allocator.allocate(0, alignment(1)).expect("zero"),
        MemoryAddress::new(0x1003)
    );
}

#[test]
fn address_exhaustion_preserves_cursor_and_storage() {
    for (start, length, required_alignment) in
        [(u64::MAX - 1, 2, 1), (u64::MAX - 1, 1, 8), (u64::MAX, 0, 1)]
    {
        // Given a cursor near the end of the address space.
        let memory = Arc::new(SharedMemory::new());
        let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
            Arc::clone(&memory),
            MemoryAddress::new(start),
        );

        // When the requested alignment or cursor advance would overflow.
        let result = allocator.allocate(length, alignment(required_alignment));

        // Then failure installs nothing and a smaller fitting request still starts at the cursor.
        assert_eq!(
            result,
            Err(hardware_interface_runtime::MemoryAllocationError::AddressExhausted)
        );
        assert_eq!(memory.revision().raw(), 0);
        assert!(memory.regions().is_empty());
        if start < u64::MAX {
            assert_eq!(
                allocator.allocate(0, alignment(1)).expect("zero"),
                MemoryAddress::new(start)
            );
        }
    }
}
