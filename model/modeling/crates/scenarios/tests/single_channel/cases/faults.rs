//! Software allocation/submission failures and independent error-event storage.

use super::{TestResult, copy};
use crate::{
    harness::{self, FaultInjection},
    software,
};

#[tokio::test(flavor = "current_thread")]
async fn successful_copy_ignores_unwritable_event_storage() -> TestResult {
    // Given: the memory endpoint rejects writes to the configured event ring.
    // When: hardware executes Copy, Fence and Trap successfully.
    copy(true, software::Input::linear(b"dma-copy")).await?;
    // Then: the shared assertions verify copied bytes, Fence completion and no fatal.
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn software_allocation_failure_stops_interrupt_runtime() -> TestResult {
    // Given: a platform allocator whose address space is exhausted.
    // When: the software program requests its own ring storage.
    let report = harness::run(
        FaultInjection::AllocationExhausted,
        software::Input::linear(b"dma-copy"),
    )
    .await?;
    // Then: its allocation error returns without installing memory or delivering an IRQ.
    assert!(matches!(
        report.program.as_ref().err().and_then(|error| {
            error.downcast_ref::<hw_model_io::memory::MemoryAllocationError>()
        }),
        Some(hw_model_io::memory::MemoryAllocationError::AddressExhausted)
    ));
    assert!(report.storage.regions()?.is_empty());
    assert!(report.worker.is_cancelled());
    assert_eq!(report.summary.deliveries(), 0);
    assert!(!report.irq);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn software_setup_failure_stops_interrupt_runtime() -> TestResult {
    // Given: the memory endpoint will reject writes to the configured command ring.
    // When: software encounters the memory error before it registers its ISR.
    let report = harness::run(
        FaultInjection::CommandWrite,
        software::Input::linear(b"dma-copy"),
    )
    .await?;
    // Then: the original software error returns and no dispatcher waits indefinitely.
    assert!(matches!(
        report.program.as_ref().err().and_then(
            |error| error.downcast_ref::<pdma_model_host::DmaError<pdma_model_dma::Error>>()
        ),
        Some(pdma_model_host::DmaError::Memory(_))
    ));
    assert_eq!(report.summary.deliveries(), 0);
    assert!(report.worker.is_cancelled());
    assert!(!report.irq);
    Ok(())
}
