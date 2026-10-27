//! Public sideband access, erasure, and binding behavior.

use std::sync::Arc;

use hardware_interface_io::memory::{
    BoundMemory, MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use hardware_interface_io::memory::{MemoryAccessError, MemoryRegion, SharedMemory};

enum Bank {
    First,
    Second,
}

struct BankMemory {
    first: SharedMemory,
    second: SharedMemory,
}

impl BankMemory {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let first = SharedMemory::new();
        let second = SharedMemory::new();
        first.add_regions(&[MemoryRegion::try_new(
            0x1000.into(),
            4,
            MemoryAlignment::ONE,
        )?])?;
        second.add_regions(&[MemoryRegion::try_new(
            0x1000.into(),
            8,
            MemoryAlignment::ONE,
        )?])?;
        first.write(0x1000.into(), MemoryAlignment::ONE, &[1, 2, 3, 4])?;
        second.write(0x1000.into(), MemoryAlignment::ONE, &[5, 6, 7, 8])?;
        Ok(Self { first, second })
    }

    const fn bank(&self, sideband: &Bank) -> &SharedMemory {
        match sideband {
            Bank::First => &self.first,
            Bank::Second => &self.second,
        }
    }
}

impl MemoryAccess<Bank> for BankMemory {
    type Error = MemoryAccessError;

    fn validate_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.bank(sideband).validate(address, alignment, length)
    }

    fn read_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.bank(sideband).read(address, alignment, destination)
    }

    fn write_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.bank(sideband).write(address, alignment, source)
    }
}

#[test]
fn tagged_trait_object_routes_raw_operations() -> Result<(), Box<dyn std::error::Error>> {
    // Given: independent banks sharing the same address range.
    let memory = BankMemory::new()?;
    let erased: &dyn MemoryAccess<Bank, Error = MemoryAccessError> = &memory;
    // When: raw operations explicitly select the second bank.
    erased.validate_with_sideband(&Bank::Second, 0x1000.into(), MemoryAlignment::ONE, 8)?;
    erased.write_with_sideband(&Bank::Second, 0x1000.into(), MemoryAlignment::ONE, &[9; 4])?;
    let mut bytes = [0; 4];
    erased.read_with_sideband(
        &Bank::Second,
        0x1000.into(),
        MemoryAlignment::ONE,
        &mut bytes,
    )?;
    // Then: the selected bytes change while the other bank retains its contents.
    assert_eq!(bytes, [9; 4]);
    assert_eq!(
        memory
            .first
            .read_buffer::<[u8; 4]>(0x1000.into(), MemoryAlignment::ONE)?,
        [1, 2, 3, 4]
    );
    Ok(())
}

#[test]
fn shared_buffer_access_keeps_non_clone_sideband() -> Result<(), Box<dyn std::error::Error>> {
    // Given: erased shared access and metadata that does not implement Clone.
    let memory = Arc::new(BankMemory::new()?);
    let erased = SharedMemoryAccess::<Bank, MemoryAccessError>::from(Arc::clone(&memory));
    let cloned = erased.clone();
    // When: buffer helpers select the second bank.
    cloned.write_buffer_with_sideband(
        &Bank::Second,
        0x1000.into(),
        MemoryAlignment::ONE,
        &[8, 7],
    )?;
    let bytes = erased.read_buffer_with_sideband::<[u8; 4]>(
        &Bank::Second,
        0x1000.into(),
        MemoryAlignment::ONE,
    )?;
    // Then: the byte extent and metadata reach the same shared backend.
    assert_eq!(bytes, [8, 7, 7, 8]);
    assert!(erased.same_handle(&cloned));
    Ok(())
}

#[test]
fn bound_memory_forwards_owned_sideband_to_unsized_backend()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a unit-memory adapter bound to a tagged trait object.
    let memory = BankMemory::new()?;
    let erased: &dyn MemoryAccess<Bank, Error = MemoryAccessError> = &memory;
    let bound = BoundMemory::new(erased, Bank::Second);
    let ordinary: &dyn MemoryAccess<Error = MemoryAccessError> = &bound;
    // When: ordinary raw and buffer operations use the binding.
    ordinary.validate(0x1000.into(), MemoryAlignment::ONE, 8)?;
    ordinary.write(0x1000.into(), MemoryAlignment::ONE, &[2, 4])?;
    let mut raw = [0; 4];
    ordinary.read(0x1000.into(), MemoryAlignment::ONE, &mut raw)?;
    let bytes = bound.read_buffer::<[u8; 4]>(0x1000.into(), MemoryAlignment::ONE)?;
    // Then: the owned selection remains fixed for all operations.
    assert_eq!(bytes, [2, 4, 7, 8]);
    assert_eq!(raw, bytes);
    assert_eq!(
        memory
            .first
            .read_buffer::<[u8; 4]>(0x1000.into(), MemoryAlignment::ONE)?,
        [1, 2, 3, 4]
    );
    Ok(())
}

#[test]
fn tagged_buffer_access_preserves_alignment_and_failed_write_atomicity()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: shared tagged access to initialized banks.
    let memory = Arc::new(BankMemory::new()?);
    let erased = SharedMemoryAccess::<Bank, MemoryAccessError>::from(Arc::clone(&memory));
    let address = MemoryAddress::new(0x1001);
    let alignment = MemoryAlignment::try_new(4)?;
    // When: explicit buffer operations request a misaligned extent in the second bank.
    let written = erased.write_buffer_with_sideband(&Bank::Second, address, alignment, &[9, 9]);
    let read = erased.read_buffer_with_sideband::<[u8; 2]>(&Bank::Second, address, alignment);
    // Then: the supplied alignment reaches the backend and failed writes preserve storage.
    assert_eq!(
        written,
        Err(MemoryAccessError::Misaligned { address, alignment })
    );
    assert_eq!(
        read,
        Err(MemoryAccessError::Misaligned { address, alignment })
    );
    assert_eq!(
        memory
            .second
            .read_buffer::<[u8; 4]>(0x1000.into(), MemoryAlignment::ONE)?,
        [5, 6, 7, 8]
    );
    Ok(())
}

#[test]
fn shared_validation_uses_selected_bank_extent() -> Result<(), Box<dyn std::error::Error>> {
    // Given: erased banks with different mapped extents.
    let erased = SharedMemoryAccess::<Bank, MemoryAccessError>::from(Arc::new(BankMemory::new()?));
    let address = MemoryAddress::new(0x1005);
    // When: the same extent is validated against each bank.
    let first = erased.validate_with_sideband(&Bank::First, address, MemoryAlignment::ONE, 1);
    let second = erased.validate_with_sideband(&Bank::Second, address, MemoryAlignment::ONE, 1);
    // Then: only the bank containing that extent accepts it.
    assert_eq!(first, Err(MemoryAccessError::Unmapped { address }));
    assert_eq!(second, Ok(()));
    Ok(())
}

#[test]
fn tagged_helpers_preserve_backend_errors() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the first bank ends before an access accepted by the second bank.
    let memory = BankMemory::new()?;
    let bound = BoundMemory::new(&memory, Bank::First);
    let address = MemoryAddress::new(0x1005);
    let expected = Err(MemoryAccessError::Unmapped { address });
    // When: validation, buffer reads and buffer writes reach that bank through adapters.
    let validated = bound.validate(address, MemoryAlignment::ONE, 1);
    let read =
        memory.read_buffer_with_sideband::<[u8; 1]>(&Bank::First, address, MemoryAlignment::ONE);
    let written = bound.write_buffer(address, MemoryAlignment::ONE, &[9]);
    // Then: all errors retain the original address and variant.
    assert_eq!(validated, expected);
    assert_eq!(read, Err(MemoryAccessError::Unmapped { address }));
    assert_eq!(written, expected);
    Ok(())
}
