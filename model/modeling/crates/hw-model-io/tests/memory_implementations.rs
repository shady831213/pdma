//! A consumer can select its own backing or a supplied model through the same API.

use hw_model_io::memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use std::{cell::RefCell, ops::Range};

#[derive(Debug, PartialEq, Eq)]
enum ArrayError {
    Range,
    Borrowed,
}

struct ArrayMemory(RefCell<[u8; 8]>);

impl ArrayMemory {
    fn range(
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<Range<usize>, ArrayError> {
        let end = address.checked_add(length).ok_or(ArrayError::Range)?;
        if !alignment.accepts(address) || end.raw() > 8 {
            return Err(ArrayError::Range);
        }
        Ok(
            usize::try_from(address.raw()).map_err(|_| ArrayError::Range)?
                ..usize::try_from(end.raw()).map_err(|_| ArrayError::Range)?,
        )
    }
}

impl MemoryAccess for ArrayMemory {
    type Error = ArrayError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        Self::range(address, alignment, length).map(|_| ())
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        let length = u64::try_from(destination.len()).map_err(|_| ArrayError::Range)?;
        let range = Self::range(address, alignment, length)?;
        let bytes = self.0.try_borrow().map_err(|_| ArrayError::Borrowed)?;
        destination.copy_from_slice(bytes.get(range).ok_or(ArrayError::Range)?);
        Ok(())
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        let length = u64::try_from(source.len()).map_err(|_| ArrayError::Range)?;
        let range = Self::range(address, alignment, length)?;
        let mut bytes = self.0.try_borrow_mut().map_err(|_| ArrayError::Borrowed)?;
        bytes
            .get_mut(range)
            .ok_or(ArrayError::Range)?
            .copy_from_slice(source);
        Ok(())
    }
}

fn transfer<M: MemoryAccess>(memory: &M, bytes: [u8; 4]) -> Result<[u8; 4], M::Error> {
    memory.write_buffer(2.into(), MemoryAlignment::ONE, &bytes)?;
    memory.read_buffer(2.into(), MemoryAlignment::ONE)
}

#[test]
fn custom_array_and_builtin_memory_supply_the_same_consumer()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: independent storage chosen by assembly, not by the consumer.
    let custom = ArrayMemory(RefCell::new([0; 8]));
    let builtin = SharedMemory::new();
    builtin.add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let payload = [7, 11, 13, 17];
    // When: the same generic consumer transfers through either implementation.
    let custom_result = transfer(&custom, payload);
    let builtin_result = transfer(&builtin, payload);
    // Then: both preserve the supplied bytes without requiring a common error type.
    assert_eq!(custom_result, Ok(payload));
    assert_eq!(builtin_result, Ok(payload));
    assert_eq!(
        custom.read(7.into(), MemoryAlignment::ONE, &mut [0; 2]),
        Err(ArrayError::Range)
    );
    Ok(())
}
