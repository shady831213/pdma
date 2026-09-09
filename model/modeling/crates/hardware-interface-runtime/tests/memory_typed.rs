//! Typed byte-buffer access inherited by existing memory backends.

mod support;

use hardware_interface_runtime::{
    LocalMemoryStorage, Memory, MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment,
    SharedMemory, SharedMemoryAccess,
};
use std::sync::Arc;
use support::{alignment, install, region};

fn exercise(memory: &impl MemoryAccess) -> Result<(), MemoryAccessError> {
    let address = MemoryAddress::new(0x1000);
    memory.write(address, MemoryAlignment::ONE, &[0x12, 0x34, 0x56, 0x78])?;
    let value = memory.read_typed::<[u8; 4]>(address, MemoryAlignment::ONE)?;
    assert_eq!(value, [0x12, 0x34, 0x56, 0x78]);
    memory.write_typed(address, MemoryAlignment::ONE, &[0xab, 0xcd])?;
    let mut bytes = [0; 4];
    memory.read(address, MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0xab, 0xcd, 0x56, 0x78]);
    Ok(())
}

#[test]
fn existing_backends_inherit_typed_access() -> Result<(), Box<dyn std::error::Error>> {
    // Given: local, shared and erased backends with only existing byte operations.
    let shared = Arc::new(SharedMemory::new());
    install(&shared, &[region(0x1000, 8, 1)]);
    let local = Memory::<LocalMemoryStorage>::new();
    local
        .prepare_regions(&[region(0x1000, 8, 1)])?
        .try_install_on(&local)?;
    let erased = SharedMemoryAccess::from(Arc::clone(&shared));
    // When / Then: typed and raw access observe the same bytes on every backend.
    exercise(shared.as_ref())?;
    exercise(&local)?;
    exercise(&erased)?;
    Ok(())
}

#[test]
fn typed_access_preserves_byte_access_errors_and_failed_write_atomicity()
-> Result<(), MemoryAccessError> {
    // Given: a full region containing sentinel bytes.
    let memory = SharedMemory::new();
    let mapped = region(0x1000, 4, 1);
    install(&memory, &[mapped]);
    memory.write(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &[9; 4])?;
    // When
    let crossing = memory.write_typed(MemoryAddress::new(0x1003), MemoryAlignment::ONE, &[1, 2]);
    let misaligned = memory.read_typed::<[u8; 2]>(MemoryAddress::new(0x1001), alignment(2));
    let unmapped = memory.read_typed::<[u8; 2]>(MemoryAddress::new(0x2000), MemoryAlignment::ONE);
    let overflow = memory.read_typed::<[u8; 2]>(MemoryAddress::new(u64::MAX), alignment(2));
    // Then: typed access preserves the byte backend's exact error and storage.
    assert_eq!(
        crossing,
        Err(MemoryAccessError::CrossesRegion {
            address: MemoryAddress::new(0x1003),
            length: 2,
            region: mapped,
        })
    );
    assert_eq!(
        misaligned,
        Err(MemoryAccessError::Misaligned {
            address: MemoryAddress::new(0x1001),
            alignment: alignment(2),
        })
    );
    assert_eq!(
        unmapped,
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0x2000)
        })
    );
    assert_eq!(
        overflow,
        Err(MemoryAccessError::AddressOverflow {
            address: MemoryAddress::new(u64::MAX),
            length: 2
        })
    );
    let mut bytes = [0; 4];
    memory.read(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [9; 4]);
    Ok(())
}

#[test]
fn zero_sized_buffers_retain_mapped_endpoint_rules() {
    // Given
    let memory = SharedMemory::new();
    install(&memory, &[region(0x1000, 4, 1)]);
    // When
    let endpoint = memory.read_typed::<[u8; 0]>(MemoryAddress::new(0x1004), MemoryAlignment::ONE);
    let gap = memory.read_typed::<[u8; 0]>(MemoryAddress::new(0x1005), MemoryAlignment::ONE);
    // Then
    assert_eq!(endpoint, Ok([]));
    assert_eq!(
        gap,
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0x1005)
        })
    );
}
