//! Contiguous transfers and byte-aligned payload storage.

use super::{TestResult, copy};
use crate::software;

#[tokio::test(flavor = "current_thread")]
async fn copy_completes_through_host_irq_delivery() -> TestResult {
    // Given: valid rings, payload memory, hardware and a software CPU.
    // When: the software submits a copy and waits for its IRQ handler.
    copy(false, software::Input::linear(b"dma-copy")).await?;
    // Then: the shared case assertions verified payload, response and cleared IRQ.
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
