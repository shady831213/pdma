//! Backends explicitly distinguish a repairable missing translation from unmapped storage.

use hardware_interface_runtime::{MemoryAccessError, MemoryAddress};

#[test]
fn missing_translation_retains_the_faulting_byte_address() {
    // Given: the first missing byte is inside an attempted access.
    let address = MemoryAddress::new(0x1080);
    // When: a backend reports a repairable translation miss.
    let error = MemoryAccessError::PageFault { address };
    // Then: the caller can recover the exact address without conflating access denial.
    assert_ne!(error, MemoryAccessError::Unmapped { address });
    assert!(error.to_string().contains("1080"));
}
