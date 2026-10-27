//! Backend-owned errors survive public memory adapters without classification.

use std::sync::Arc;

use hardware_interface_io::memory::{
    BoundMemory, MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use hardware_interface_io::memory::{MemoryAccessError, MemoryRegion, SharedMemory};

#[derive(Debug, PartialEq, Eq)]
enum BackendError {
    Rejected { address: MemoryAddress, key: u16 },
    Storage(MemoryAccessError),
}

struct KeyedMemory(SharedMemory);

impl KeyedMemory {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let memory = SharedMemory::new();
        let region = MemoryRegion::try_new(0x1000.into(), 8, MemoryAlignment::ONE)?;
        memory.add_regions(&[region])?;
        memory.write(0x1000.into(), MemoryAlignment::ONE, &[1; 8])?;
        Ok(Self(memory))
    }

    const fn authorize(key: u16, address: MemoryAddress) -> Result<(), BackendError> {
        match key {
            7 => Ok(()),
            key => Err(BackendError::Rejected { address, key }),
        }
    }
}

impl MemoryAccess<u16> for KeyedMemory {
    type Error = BackendError;

    fn validate_with_sideband(
        &self,
        key: &u16,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        Self::authorize(*key, address)?;
        self.0
            .validate(address, alignment, length)
            .map_err(BackendError::Storage)
    }

    fn read_with_sideband(
        &self,
        key: &u16,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        Self::authorize(*key, address)?;
        self.0
            .read(address, alignment, destination)
            .map_err(BackendError::Storage)
    }

    fn write_with_sideband(
        &self,
        key: &u16,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        Self::authorize(*key, address)?;
        self.0
            .write(address, alignment, source)
            .map_err(BackendError::Storage)
    }
}

#[test]
fn shared_and_bound_access_reaches_real_storage() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a backend with a non-Clone error that does not implement std::error::Error.
    let memory = Arc::new(KeyedMemory::new()?);
    let shared = SharedMemoryAccess::<u16, BackendError>::from(Arc::clone(&memory));
    let cloned = shared.clone();
    let bound = BoundMemory::new(&cloned, 7);
    // When: buffer bytes are written through a bound shared capability.
    let written = bound.write_buffer(0x1002.into(), MemoryAlignment::ONE, &[2, 3, 4]);
    // Then: reads through the other capability observe exactly those bytes and their neighbors.
    assert_eq!(written, Ok(()));
    assert_eq!(
        shared.read_buffer_with_sideband::<[u8; 8]>(&7, 0x1000.into(), MemoryAlignment::ONE),
        Ok([1, 1, 2, 3, 4, 1, 1, 1])
    );
    assert!(shared.same_handle(&cloned));
    Ok(())
}

#[test]
fn bound_unsized_backend_preserves_raw_rejections() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an erased backend bound to a rejected key.
    let memory = KeyedMemory::new()?;
    let erased: &dyn MemoryAccess<u16, Error = BackendError> = &memory;
    let bound = BoundMemory::new(erased, 19);
    let address = MemoryAddress::new(0x1003);
    let mut destination = [8; 2];
    // When: each raw operation reaches the rejected binding.
    let results = [
        bound.validate(address, MemoryAlignment::ONE, 2),
        bound.read(address, MemoryAlignment::ONE, &mut destination),
        bound.write(address, MemoryAlignment::ONE, &[9, 9]),
    ];
    // Then: all operations preserve the backend's exact address and key.
    for result in results {
        assert_eq!(result, Err(BackendError::Rejected { address, key: 19 }));
    }
    assert_eq!(destination, [8; 2]);
    assert_eq!(
        memory
            .0
            .read_buffer::<[u8; 8]>(0x1000.into(), MemoryAlignment::ONE),
        Ok([1; 8])
    );
    Ok(())
}

#[test]
fn shared_buffer_helpers_preserve_rejection_payload() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a shared custom backend and an address inside its real storage.
    let shared = SharedMemoryAccess::<u16, BackendError>::from(Arc::new(KeyedMemory::new()?));
    let address = MemoryAddress::new(0x1005);
    let bound = BoundMemory::new(&shared, 23);
    // When: validation and tagged/unit buffer helpers use a rejected key.
    let results = [
        shared.validate_with_sideband(&23, address, MemoryAlignment::ONE, 1),
        shared.write_buffer_with_sideband(&23, address, MemoryAlignment::ONE, &[9]),
        bound.write_buffer(address, MemoryAlignment::ONE, &[9]),
    ];
    let tagged = shared.read_buffer_with_sideband::<[u8; 1]>(&23, address, MemoryAlignment::ONE);
    let ordinary = bound.read_buffer::<[u8; 1]>(address, MemoryAlignment::ONE);
    // Then: helpers return the original backend error, including the exact payload.
    for result in results {
        assert_eq!(result, Err(BackendError::Rejected { address, key: 23 }));
    }
    assert_eq!(tagged, Err(BackendError::Rejected { address, key: 23 }));
    assert_eq!(ordinary, Err(BackendError::Rejected { address, key: 23 }));
    Ok(())
}

#[test]
fn backend_storage_error_remains_distinct_from_rejection() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: an authorized bound shared capability and an unmapped address.
    let shared = SharedMemoryAccess::<u16, BackendError>::from(Arc::new(KeyedMemory::new()?));
    let bound = BoundMemory::new(&shared, 7);
    let address = MemoryAddress::new(0x1080);
    // When: the backend checks the actual backing extent.
    let result = bound.read_buffer::<[u8; 1]>(address, MemoryAlignment::ONE);
    // Then: the backing error remains distinct and retains the access address.
    assert_eq!(
        result,
        Err(BackendError::Storage(MemoryAccessError::Unmapped {
            address
        }))
    );
    Ok(())
}
