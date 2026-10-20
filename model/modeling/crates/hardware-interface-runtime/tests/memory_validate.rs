#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use std::sync::Arc;

use hardware_interface_runtime::{
    LocalMemoryStorage, Memory, MemoryAccess, MemoryAccessError, MemoryAddress, SharedMemory,
    SharedMemoryAccess,
};
use support::{alignment, install, region};

#[test]
fn validate_preserves_error_precedence_and_zero_endpoints() {
    // Given adjacent regions and probes covering every resolution outcome.
    let memory = SharedMemory::new();
    install(&memory, &[region(0x1000, 64, 64), region(0x1040, 64, 64)]);
    let cases = [
        (0x1000, 64, 64, Ok(())),
        (0x1040, 64, 0, Ok(())),
        (0x1080, 64, 0, Ok(())),
        (
            u64::MAX,
            8,
            2,
            Err(MemoryAccessError::AddressOverflow {
                address: MemoryAddress::new(u64::MAX),
                length: 2,
            }),
        ),
        (
            3,
            8,
            1,
            Err(MemoryAccessError::Misaligned {
                address: MemoryAddress::new(3),
                alignment: alignment(8),
            }),
        ),
        (
            0x0ff8,
            8,
            16,
            Err(MemoryAccessError::Unmapped {
                address: MemoryAddress::new(0x0ff8),
            }),
        ),
        (
            0x2000,
            1,
            0,
            Err(MemoryAccessError::Unmapped {
                address: MemoryAddress::new(0x2000),
            }),
        ),
        (
            0x103f,
            1,
            2,
            Err(MemoryAccessError::CrossesRegion {
                address: MemoryAddress::new(0x103f),
                length: 2,
                region: region(0x1000, 64, 64),
            }),
        ),
    ];

    for (address, required_alignment, length, expected) in cases {
        // When the complete extent is checked without a caller byte buffer.
        let result = memory.validate(
            MemoryAddress::new(address),
            alignment(required_alignment),
            length,
        );

        // Then overflow, alignment, lookup, and crossing retain their original precedence.
        assert_eq!(result, expected);
    }
}

#[test]
fn validate_through_shared_capability_preserves_bytes_and_regions() {
    // Given a shared capability over initialized backing.
    let memory = Arc::new(SharedMemory::new());
    install(&memory, &[region(0x1000, 64, 64)]);
    memory
        .write(MemoryAddress::new(0x1000), alignment(1), &[0xa5; 64])
        .expect("sentinel");
    let regions = memory.regions().expect("metadata is accessible");
    let access = SharedMemoryAccess::from(Arc::clone(&memory));

    // When validation traverses the erased capability.
    let result = access.validate(MemoryAddress::new(0x1000), alignment(64), 64);

    // Then validation leaves every observable storage property unchanged.
    assert_eq!(result, Ok(()));
    assert_eq!(memory.regions().expect("metadata is accessible"), regions);
    let mut bytes = [0; 64];
    memory
        .read(MemoryAddress::new(0x1000), alignment(1), &mut bytes)
        .expect("read sentinel");
    assert_eq!(bytes, [0xa5; 64]);
}

#[test]
fn validate_is_available_on_local_memory() {
    // Given the local storage policy with a committed region.
    let memory = Memory::<LocalMemoryStorage>::new();
    memory
        .add_regions(&[region(0x1000, 64, 64)])
        .expect("regions install");

    // When the generic memory implementation validates a mapped endpoint.
    let result = memory.validate(MemoryAddress::new(0x1040), alignment(64), 0);

    // Then local memory shares the same validation behavior.
    assert_eq!(result, Ok(()));
}
