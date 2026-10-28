#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use hw_model_io::memory::{
    MemoryAddress, MemoryAllocationError, SharedMemory, SharedMemoryAllocator,
};
use std::sync::Arc;
use support::{alignment, region};

#[test]
fn constructor_rejects_overflow_without_installing_backing() {
    // Given a range whose exclusive end cannot be represented.
    let memory = Arc::new(SharedMemory::new());
    // When constructing the allocator.
    let result = SharedMemoryAllocator::new(Arc::clone(&memory), u64::MAX.into(), 1);
    // Then the invalid range is distinguished from allocation exhaustion.
    assert!(matches!(result, Err(MemoryAllocationError::InvalidRange)));
    assert!(memory.regions().expect("regions").is_empty());
}

#[test]
fn exact_fit_including_padding_exhausts_cloned_allocator() {
    // Given a heap containing three padding bytes and a four-byte allocation.
    let memory = Arc::new(SharedMemory::new());
    let allocator =
        SharedMemoryAllocator::new(Arc::clone(&memory), 0x1001.into(), 7).expect("valid heap");
    let other = allocator.clone();
    assert!(memory.regions().expect("lazy backing").is_empty());
    // When one clone fills the aligned remainder.
    let address = other.allocate(4, alignment(4)).expect("exact fit");
    // Then both handles share the exhausted limit and only requested backing exists.
    assert_eq!(address, MemoryAddress::new(0x1004));
    assert_eq!(
        allocator.allocate(1, alignment(1)),
        Err(MemoryAllocationError::AddressExhausted)
    );
    assert_eq!(
        other.allocate(0, alignment(1)),
        Err(MemoryAllocationError::AddressExhausted)
    );
    assert_eq!(memory.regions().expect("regions"), [region(0x1004, 4, 4)]);
}

#[test]
fn exhausted_request_preserves_padding_and_cursor() {
    for (size, length, align) in [(7, 5, 4), (3, 1, 4), (2, 0, 4), (7, u64::MAX, 1)] {
        // Given an unaligned heap too small for the requested extent or padding.
        let memory = Arc::new(SharedMemory::new());
        let allocator = SharedMemoryAllocator::new(Arc::clone(&memory), 0x1001.into(), size)
            .expect("valid heap");
        // When the allocation exceeds that heap.
        let result = allocator.allocate(length, alignment(align));
        // Then no backing or padding is consumed, and the full original heap still fits.
        assert_eq!(result, Err(MemoryAllocationError::AddressExhausted));
        assert!(memory.regions().expect("regions").is_empty());
        assert_eq!(
            allocator.allocate(size, alignment(1)).expect("retry"),
            MemoryAddress::new(0x1001)
        );
    }
}

#[test]
fn empty_heap_rejects_even_zero_length_allocations() {
    for base in [0, 0x1003, u64::MAX] {
        // Given an empty but representable heap.
        let memory = Arc::new(SharedMemory::new());
        let allocator =
            SharedMemoryAllocator::new(Arc::clone(&memory), base.into(), 0).expect("empty heap");
        // When reserving either zero bytes or one byte.
        for length in [0, 1] {
            // Then neither request can consume address space or install backing.
            assert_eq!(
                allocator.allocate(length, alignment(1)),
                Err(MemoryAllocationError::AddressExhausted)
            );
        }
        assert!(memory.regions().expect("regions").is_empty());
    }
}

#[test]
fn zero_length_consumes_padding_and_one_byte_of_heap() {
    // Given three padding bytes and exactly one logical address reservation.
    let memory = Arc::new(SharedMemory::with_region_capacity(0));
    let allocator =
        SharedMemoryAllocator::new(Arc::clone(&memory), 0x1001.into(), 4).expect("valid heap");
    // When reserving an aligned zero-length allocation.
    let address = allocator.allocate(0, alignment(4)).expect("zero length");
    // Then it consumes the whole heap without installing backing.
    assert_eq!(address, MemoryAddress::new(0x1004));
    assert_eq!(
        allocator.allocate(0, alignment(1)),
        Err(MemoryAllocationError::AddressExhausted)
    );
    assert!(memory.regions().expect("regions").is_empty());
}

#[test]
fn alignment_overflow_preserves_last_representable_extent() {
    // Given the final representable one-byte heap.
    let memory = Arc::new(SharedMemory::new());
    let base = MemoryAddress::new(u64::MAX - 1);
    let allocator = SharedMemoryAllocator::new(Arc::clone(&memory), base, 1).expect("valid heap");
    // When aligning beyond the address space.
    let result = allocator.allocate(1, alignment(8));
    // Then failure leaves the last byte available with its original address.
    assert_eq!(result, Err(MemoryAllocationError::AddressExhausted));
    assert!(memory.regions().expect("regions").is_empty());
    assert_eq!(
        allocator.allocate(1, alignment(1)).expect("last byte"),
        base
    );
}
