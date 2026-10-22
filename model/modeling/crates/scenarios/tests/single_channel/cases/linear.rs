//! Contiguous transfers and byte-aligned payload storage.

use super::{TestResult, copy};
use crate::software;

#[tokio::test(flavor = "current_thread")]
async fn copy_completes_through_host_irq_delivery() -> TestResult {
    // Given: valid rings, payload memory, hardware and a software CPU.
    // When: the software submits a copy and waits for its IRQ handler.
    copy(false, software::Input::linear(b"dma-copy")).await?;
    // Then: the shared assertions verify payload, Fence and acknowledged Trap.
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn successful_copy_does_not_publish_an_error_event() -> TestResult {
    // Given: a valid copy submitted through the software channel driver.
    let input = software::Input::linear(b"event-free");
    // When: hardware executes it and software observes its notification.
    let report = crate::harness::run(crate::harness::FaultInjection::None, input).await?;
    let outcome = report.program?;
    // Then: successful work does not consume error-event storage.
    assert_eq!(outcome.event, None);
    assert!(outcome.completed);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn copy_accepts_a_byte_aligned_destination() -> TestResult {
    // Given: a nine-byte payload, placing the next byte-aligned allocation off an eight-byte boundary.
    let data = b"unaligned";
    // When: software allocates its buffers and submits the copy.
    let destination = copy(false, software::Input::linear(data)).await?;
    // Then: copying and IRQ delivery succeeded at an actually unaligned destination.
    let span = destination.first().ok_or("linear destination missing")?;
    assert_ne!(span.address % 8, 0);
    Ok(())
}
