use super::memory;
use crate::TestResult;
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOrder, AtomicReadOrder, AtomicWriteOrder, MemoryAccess,
    MemoryAccessError, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{WaitCondition, WaitOrder};
use pdma_model_software::signal::Signal;
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

#[test]
fn invalid_atomic_addresses_leave_all_backing_bytes_unchanged() -> TestResult {
    // Given: a complete region plus a shorter region that cannot contain a signal.
    let memory = memory()?;
    let short_region = MemoryRegion::try_new(0x2000.into(), 4, MemoryAlignment::ONE)?;
    memory.add_regions(&[short_region])?;
    let original = [0x35; 16];
    memory.write(0x1000.into(), MemoryAlignment::ONE, &original)?;
    memory.write(0x2000.into(), MemoryAlignment::ONE, &[0x71; 4])?;
    // When: unaligned, unmapped, and region-crossing updates are attempted.
    for (address, expected) in [
        (
            0x1001,
            MemoryAccessError::Misaligned {
                address: 0x1001.into(),
                alignment: MemoryAlignment::try_new(8)?,
            },
        ),
        (
            0x3000,
            MemoryAccessError::Unmapped {
                address: 0x3000.into(),
            },
        ),
        (
            0x2000,
            MemoryAccessError::CrossesRegion {
                address: 0x2000.into(),
                length: 8,
                region: short_region,
            },
        ),
    ] {
        let result = Signal::new(&memory, address.into()).add(-1, AtomicOrder::Release);
        // Then: the backend error remains typed and no ordinary bytes changed.
        assert_eq!(result, Err(AtomicAccessError::Access(expected)));
        let mut bytes = [0; 16];
        memory.read(0x1000.into(), MemoryAlignment::ONE, &mut bytes)?;
        assert_eq!(bytes, original);
        let mut short = [0; 4];
        memory.read(0x2000.into(), MemoryAlignment::ONE, &mut short)?;
        assert_eq!(short, [0x71; 4]);
    }
    Ok(())
}

struct ByteOnly(SharedMemory);

impl MemoryAccess for ByteOnly {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.0.validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.0.read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.0.write(address, alignment, bytes)
    }
}

#[test]
fn unsupported_atomics_never_fall_back_to_separate_read_and_write() -> TestResult {
    // Given: an ordinary read/write backend intentionally exposing no atomic capability.
    let memory = ByteOnly(memory()?);
    let initial = -19_i64;
    memory.write(0x1000.into(), MemoryAlignment::ONE, &initial.to_le_bytes())?;
    let signal = Signal::new(&memory, 0x1000.into());
    // When: every signal operation requests the unsupported atomic capability.
    let results = [
        signal.load(AtomicReadOrder::Acquire),
        signal.store(7, AtomicWriteOrder::Release).map(|()| 0),
        signal.add(7, AtomicOrder::Relaxed),
        signal.subtract(7, AtomicOrder::Acquire),
        signal.and(7, AtomicOrder::Release),
        signal.or(7, AtomicOrder::AcquireRelease),
        signal.xor(7, AtomicOrder::Relaxed),
        signal.exchange(7, AtomicOrder::Acquire),
        signal.compare_exchange(initial, 7, AtomicOrder::AcquireRelease),
    ];
    let waiting = signal.try_wait(WaitCondition::Equal, initial, WaitOrder::Acquire);
    let mut wait = std::pin::pin!(signal.wait(WaitCondition::Equal, initial, WaitOrder::Acquire));
    let polled = wait.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    // Then: every operation rejects access and preserves the original bytes.
    for result in results {
        assert_eq!(result, Err(AtomicAccessError::Unsupported));
    }
    assert_eq!(waiting, Err(AtomicAccessError::Unsupported));
    assert!(matches!(
        polled,
        Poll::Ready(Err(AtomicAccessError::Unsupported))
    ));
    let mut bytes = [0; 8];
    memory.read(0x1000.into(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(i64::from_le_bytes(bytes), initial);
    Ok(())
}
