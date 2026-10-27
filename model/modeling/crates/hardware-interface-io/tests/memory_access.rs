#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use hardware_interface_io::memory::{MemoryAccess, MemoryAddress};
use hardware_interface_io::memory::{MemoryAccessError, SharedMemory};
use support::{alignment, install, region};

#[test]
fn memory_access_accepts_zero_length_at_left_shared_endpoint_and_final_end() {
    // Given adjacent regions whose shared endpoint belongs to the earlier region for zero bytes.
    let memory = SharedMemory::new();
    install(&memory, &[region(0x1000, 64, 64), region(0x1040, 64, 64)]);

    // When zero bytes are accessed at the shared endpoint and final end.
    let shared = memory.read(MemoryAddress::new(0x1040), alignment(64), &mut []);
    let final_end = memory.write(MemoryAddress::new(0x1080), alignment(64), &[]);
    let gap = memory.read(MemoryAddress::new(0x2000), alignment(1), &mut []);

    // Then mapped endpoints succeed and a gap remains unmapped.
    assert_eq!(shared, Ok(()));
    assert_eq!(final_end, Ok(()));
    assert_eq!(
        gap,
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0x2000),
        })
    );
}

#[test]
fn memory_access_checks_overflow_then_misalignment_before_lookup() {
    // Given an empty store and addresses rejected before region lookup.
    let memory = SharedMemory::new();

    // When one access overflows and another is both misaligned and unmapped.
    let overflow = memory.read(MemoryAddress::new(u64::MAX), alignment(8), &mut [0, 0]);
    let misaligned = memory.write(MemoryAddress::new(3), alignment(8), &[0]);

    // Then overflow has first precedence and alignment has second precedence.
    assert_eq!(
        overflow,
        Err(MemoryAccessError::AddressOverflow {
            address: MemoryAddress::new(u64::MAX),
            length: 2,
        })
    );
    assert_eq!(
        misaligned,
        Err(MemoryAccessError::Misaligned {
            address: MemoryAddress::new(3),
            alignment: alignment(8),
        })
    );
}

#[test]
fn memory_access_checks_unmapped_before_cross_region() {
    // Given one installed region with sentinel bytes.
    let memory = SharedMemory::new();
    install(&memory, &[region(0x1000, 64, 64)]);
    memory
        .write(MemoryAddress::new(0x1000), alignment(1), &[0xaa, 0xbb])
        .expect("sentinel write");

    // When an aligned access starts outside and another crosses the mapped end.
    let unmapped = memory.read(MemoryAddress::new(0x0ff8), alignment(8), &mut [0; 16]);
    let crossing = memory.write(MemoryAddress::new(0x103f), alignment(1), &[0xcc, 0xdd]);

    // Then lookup and crossing are distinct and neither changes stored bytes.
    assert_eq!(
        unmapped,
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0x0ff8),
        })
    );
    assert_eq!(
        crossing,
        Err(MemoryAccessError::CrossesRegion {
            address: MemoryAddress::new(0x103f),
            length: 2,
            region: region(0x1000, 64, 64),
        })
    );
    let mut bytes = [0; 2];
    memory
        .read(MemoryAddress::new(0x1000), alignment(1), &mut bytes)
        .expect("sentinel read");
    assert_eq!(bytes, [0xaa, 0xbb]);
}

#[test]
fn memory_failed_read_preserves_caller_destination() {
    // Given a caller destination with nonzero bytes and one mapped region.
    let memory = SharedMemory::new();
    install(&memory, &[region(0x1000, 64, 64)]);
    let mut destination = [0x55, 0x66];

    // When the read starts at a misaligned mapped address.
    let result = memory.read(MemoryAddress::new(0x1001), alignment(8), &mut destination);

    // Then the typed error is returned before copying any destination byte.
    assert_eq!(
        result,
        Err(MemoryAccessError::Misaligned {
            address: MemoryAddress::new(0x1001),
            alignment: alignment(8),
        })
    );
    assert_eq!(destination, [0x55, 0x66]);
}
