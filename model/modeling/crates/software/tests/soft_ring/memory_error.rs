use super::{CursorBus, Item};
use hardware_interface_memory::{MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_software::ring::{
    RegisterRingIo, RingConfig, RingCursorRegisters, SoftRing, SoftRingError,
};

#[derive(Debug, PartialEq, Eq)]
struct BackendError {
    address: MemoryAddress,
    reason: u32,
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "memory rejected at {}: {}",
            self.address.raw(),
            self.reason
        )
    }
}

impl std::error::Error for BackendError {}

struct RejectedMemory;

impl MemoryAccess for RejectedMemory {
    type Error = BackendError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: u64,
    ) -> Result<(), Self::Error> {
        Err(BackendError {
            address,
            reason: 71,
        })
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: &mut [u8],
    ) -> Result<(), Self::Error> {
        Err(BackendError {
            address,
            reason: 72,
        })
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: &[u8],
    ) -> Result<(), Self::Error> {
        Err(BackendError {
            address,
            reason: 73,
        })
    }
}

#[tokio::test]
async fn entry_failure_retains_backend_details_and_source() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: valid register cursors and a backend rejecting entry storage with its own error.
    let bus = CursorBus::default();
    bus.memory
        .add_regions(&[MemoryRegion::try_new(0.into(), 8, MemoryAlignment::ONE)?])?;
    let io = RegisterRingIo::new(
        &bus,
        &RejectedMemory,
        RingCursorRegisters {
            producer: 0,
            consumer: 4,
        },
    );
    let mut ring = SoftRing::new(RingConfig::<Item>::new(16, 2)?);
    // When: publication reaches the backend write.
    let error = ring.try_push(&io, Item { value: 9 }).await.unwrap_err();
    // Then: the exact typed cause is retained, and the cursor remains unpublished.
    assert!(std::error::Error::source(&error).is_some());
    assert!(
        matches!(error, SoftRingError::Memory(BackendError { address, reason: 73 }) if address.raw() == 16)
    );
    assert_eq!(
        bus.memory
            .read_typed::<[u8; 4]>(0.into(), MemoryAlignment::ONE)?,
        [0; 4]
    );
    assert!(ring.has_pending_push());
    Ok(())
}
