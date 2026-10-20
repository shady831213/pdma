//! Software allocation/submission failures and hardware response publication failure.

use super::{TestResult, copy};
use crate::{
    harness::{self, FaultInjection},
    software,
};

#[tokio::test(flavor = "current_thread")]
async fn response_write_fault_reaches_host_without_a_response_entry() -> TestResult {
    // Given: the memory endpoint will reject writes to the configured response ring.
    // When: hardware copies the payload but cannot publish its response.
    copy(true, software::Input::linear(b"dma-copy")).await?;
    // Then: the shared case assertions verified copied bytes, typed sticky fault and masked IRQ.
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
            error.downcast_ref::<hardware_interface_runtime::MemoryAllocationError>()
        }),
        Some(hardware_interface_runtime::MemoryAllocationError::AddressExhausted)
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
