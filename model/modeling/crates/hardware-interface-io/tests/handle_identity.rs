//! Capability identity is separate from backing-byte aliasing.
use hardware_interface_io::memory::{LocalMemory, MemoryRegion, SharedMemory};
use hardware_interface_io::memory::{MemoryAccess, MemoryAlignment, SharedMemoryAccess};
use std::sync::Arc;

#[test]
fn independently_wrapped_shared_memory_aliases_bytes_without_handle_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 4, MemoryAlignment::ONE)?])?;
    let first = SharedMemoryAccess::from(Arc::new(memory.clone()));
    let second = SharedMemoryAccess::from(Arc::new(memory));
    let cloned = first.clone();
    first.write_buffer(0.into(), MemoryAlignment::ONE, &42_u32.to_le_bytes())?;
    let observed = second.read_buffer::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?;
    assert_eq!(u32::from_le_bytes(observed), 42);
    assert!(!first.same_handle(&second));
    assert!(first.same_handle(&cloned));
    Ok(())
}

#[test]
fn local_memory_alias_supports_the_same_byte_buffer_api() -> Result<(), Box<dyn std::error::Error>>
{
    let memory = LocalMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(0.into(), 4, MemoryAlignment::ONE)?])?;
    memory.write_buffer(0.into(), MemoryAlignment::ONE, &[1, 2, 3, 4])?;
    assert_eq!(
        memory.read_buffer::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?,
        [1, 2, 3, 4]
    );
    Ok(())
}
