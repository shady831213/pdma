//! Atomic capability forwarding and rejection at memory boundaries.

use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, AtomicReadOrder, AtomicWriteOrder, BoundMemory,
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
    SharedMemoryAccess,
};
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
enum Bank {
    First,
    Second,
}
struct Banks([SharedMemory; 2]);
impl Banks {
    const fn bank(&self, bank: Bank) -> &SharedMemory {
        let [first, second] = &self.0;
        match bank {
            Bank::First => first,
            Bank::Second => second,
        }
    }
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let banks = Self([SharedMemory::new(), SharedMemory::new()]);
        for bank in &banks.0 {
            bank.add_regions(&[MemoryRegion::try_new(
                0x1000.into(),
                8,
                MemoryAlignment::ONE,
            )?])?;
        }
        Ok(banks)
    }
}
impl MemoryAccess<Bank> for Banks {
    type Error = MemoryAccessError;
    fn atomic32_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        self.bank(*sideband).atomic32(address, operation)
    }
    fn atomic64_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.bank(*sideband).atomic64(address, operation)
    }
    fn validate_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.bank(*sideband).validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.bank(*sideband).read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        sideband: &Bank,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.bank(*sideband).write(address, alignment, bytes)
    }
}

#[test]
fn erased_bound_atomics_preserve_sideband_and_values() -> TestResult {
    // Given: distinct words at the same address in separately selected banks.
    let banks = Arc::new(Banks::new()?);
    let untouched = 0x1122_3344_5566_7788_u64;
    banks.bank(Bank::First).write(
        0x1000.into(),
        MemoryAlignment::ONE,
        &untouched.to_le_bytes(),
    )?;
    let shared = SharedMemoryAccess::<Bank>::from(Arc::clone(&banks));
    let bound = BoundMemory::new(&shared, Bank::Second);
    let value = 0x1234_5678_u64;
    // When: store and load pass through both erasure and binding adapters.
    assert_eq!(
        bound.atomic64(
            0x1000.into(),
            AtomicOperation::Store {
                value,
                order: AtomicWriteOrder::Release
            }
        )?,
        0
    );
    let low = bound.atomic32(
        0x1000.into(),
        AtomicOperation::Load(AtomicReadOrder::Acquire),
    )?;
    // Then: only the chosen bank changes and word width is preserved.
    assert_eq!(u64::from(low), value);
    assert_eq!(
        banks.bank(Bank::First).atomic64(
            0x1000.into(),
            AtomicOperation::Load(AtomicReadOrder::Relaxed)
        )?,
        untouched
    );
    assert_eq!(
        bound.atomic64(
            0x1008.into(),
            AtomicOperation::Load(AtomicReadOrder::Relaxed)
        ),
        Err(AtomicAccessError::Access(MemoryAccessError::Unmapped {
            address: 0x1008.into()
        }))
    );
    Ok(())
}

#[test]
fn word_crossing_a_region_is_rejected_before_modification() -> TestResult {
    // Given: only half of an aligned double-word is mapped.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        0x1000.into(),
        4,
        MemoryAlignment::ONE,
    )?])?;
    let original = 0x9876_5432_u32;
    memory.atomic32(
        0x1000.into(),
        AtomicOperation::Store {
            value: original,
            order: AtomicWriteOrder::Relaxed,
        },
    )?;
    // When: a double-word store crosses the region boundary.
    let result = memory.atomic64(
        0x1000.into(),
        AtomicOperation::Store {
            value: u64::MAX,
            order: AtomicWriteOrder::Release,
        },
    );
    // Then: the exact boundary failure is preserved and no prefix is written.
    assert!(matches!(
        result,
        Err(AtomicAccessError::Access(
            MemoryAccessError::CrossesRegion { .. }
        ))
    ));
    assert_eq!(
        memory.atomic32(
            0x1000.into(),
            AtomicOperation::Load(AtomicReadOrder::Acquire)
        )?,
        original
    );
    Ok(())
}

struct ByteOnly(std::cell::Cell<usize>);

impl ByteOnly {
    fn access(&self) -> Result<(), MemoryAccessError> {
        self.0.set(self.0.get() + 1);
        Err(MemoryAccessError::Unmapped {
            address: 0x1000.into(),
        })
    }
}
impl MemoryAccess for ByteOnly {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        (): &(),
        _: MemoryAddress,
        _: MemoryAlignment,
        _: u64,
    ) -> Result<(), Self::Error> {
        self.access()
    }
    fn read_with_sideband(
        &self,
        (): &(),
        _: MemoryAddress,
        _: MemoryAlignment,
        _: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.access()
    }
    fn write_with_sideband(
        &self,
        (): &(),
        _: MemoryAddress,
        _: MemoryAlignment,
        _: &[u8],
    ) -> Result<(), Self::Error> {
        self.access()
    }
}

#[test]
fn unsupported_backend_never_emulates_an_atomic_with_separate_accesses() {
    // Given: an ordinary byte-access capability without atomic support.
    let memory = ByteOnly(std::cell::Cell::new(0));
    // When: both atomic widths are requested.
    let word = memory.atomic32(
        0x1000.into(),
        AtomicOperation::Load(AtomicReadOrder::Relaxed),
    );
    let double = memory.atomic64(
        0x1000.into(),
        AtomicOperation::Load(AtomicReadOrder::Relaxed),
    );
    // Then: neither path invokes ordinary access methods.
    assert_eq!(word, Err(AtomicAccessError::Unsupported));
    assert_eq!(double, Err(AtomicAccessError::Unsupported));
    assert_eq!(memory.0.get(), 0);
}
