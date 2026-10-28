use super::{LocalMemoryPolicy, Memory, MemoryPolicy, SharedMemoryPolicy};
use crate::memory::{
    AtomicAccessError, AtomicOperation, AtomicOrder, AtomicUpdate, MemoryAccess, MemoryAddress,
    MemoryAlignment,
};
use crate::memory::{MemoryAccessError, MemoryError, MemoryRegion, StorageBackend, StorageError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn nested_access<P: MemoryPolicy>() -> TestResult {
    let memory = Memory::<P>::new();
    let address = MemoryAddress::new(0x1000);
    let region = MemoryRegion::try_new(address, 16, MemoryAlignment::DOUBLE_WORD)?;
    let addition = MemoryRegion::try_new(MemoryAddress::new(0x2000), 16, MemoryAlignment::ONE)?;
    memory.add_regions(&[region])?;
    memory.write(address, MemoryAlignment::ONE, &[7; 8])?;

    // Given an already active storage access, nested memory access must reject.
    for mutable in [false, true] {
        let rejected = || {
            let mut bytes = [3; 8];
            let access_error = Err(MemoryAccessError::Storage(StorageError::Reentrant));
            assert_eq!(
                memory.read(address, MemoryAlignment::ONE, &mut bytes),
                access_error
            );
            assert_eq!(bytes, [3; 8]);
            assert_eq!(
                memory.write(address, MemoryAlignment::ONE, &[9; 8]),
                access_error
            );
            assert_eq!(
                memory.validate(address, MemoryAlignment::ONE, 8),
                access_error
            );
            assert_eq!(memory.regions(), Err(StorageError::Reentrant));
            assert_eq!(
                memory.add_regions(&[addition]),
                Err(MemoryError::Storage(StorageError::Reentrant))
            );
            assert_eq!(memory.add_regions(&[]), Ok(()));
            assert_eq!(
                memory.atomic32(
                    address,
                    AtomicOperation::Update {
                        update: AtomicUpdate::Add(1),
                        order: AtomicOrder::Relaxed,
                    }
                ),
                Err(AtomicAccessError::Access(MemoryAccessError::Storage(
                    StorageError::Reentrant
                )))
            );
            assert_eq!(
                memory.atomic64(
                    address,
                    AtomicOperation::Update {
                        update: AtomicUpdate::Exchange(0),
                        order: AtomicOrder::Release,
                    }
                ),
                Err(AtomicAccessError::Access(MemoryAccessError::Storage(
                    StorageError::Reentrant
                )))
            );
        };
        if mutable {
            memory.backend.mutate(|_| rejected())?;
        } else {
            memory.backend.inspect(|_| rejected())?;
        }
    }

    // Then the guard is released and stored bytes remain accessible.
    let mut bytes = [0; 8];
    memory.read(address, MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [7; 8]);
    assert_eq!(memory.regions()?, [region]);
    memory.add_regions(&[addition])?;
    memory.write(address, MemoryAlignment::ONE, &[11; 8])?;
    assert_eq!(
        memory.atomic64(
            address,
            AtomicOperation::Load(crate::memory::AtomicReadOrder::Acquire)
        )?,
        u64::from_le_bytes([11; 8])
    );
    Ok(())
}

#[test]
fn local_memory_reentry_returns_without_parking() -> TestResult {
    nested_access::<LocalMemoryPolicy>()
}

#[test]
fn shared_memory_reentry_returns_without_parking() -> TestResult {
    nested_access::<SharedMemoryPolicy>()
}
