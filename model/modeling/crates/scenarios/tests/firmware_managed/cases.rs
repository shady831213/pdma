use crate::harness::runtime::AccessError;
use crate::harness::{self, Input, TestResult};
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use std::sync::Arc;

mod contexts;
mod functions;
mod multi;
mod page_fault;

#[tokio::test(flavor = "current_thread")]
async fn relocated_platform_runs_the_same_host_and_firmware_programs() -> TestResult {
    // Given: every register block has a different platform-selected base.
    let addresses = pdma_model_software::control::ManagementAddresses {
        dma: 0x8000,
        windows: 0x5000,
        control: 0x100,
        firmware: 0x7000,
        contexts: None,
    };
    let payload = b"relocated-dma";
    // When: the ordinary software programs run on that placement.
    let report = harness::run_at(Input::Copy(payload), addresses).await?;
    let outcome = report.program?;
    let mut copied = vec![0; payload.len()];
    report
        .storage
        .read(outcome.destination, MemoryAlignment::ONE, &mut copied)?;
    // Then: boot, grant, IRQ-driven copy and release all use the supplied addresses.
    assert_eq!(copied, payload);
    assert_eq!(outcome.completion, 37);
    assert_eq!(outcome.released, 0);
    assert_eq!(outcome.dma_interrupts, 1);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn firmware_opens_access_for_the_same_host_program_and_real_irq_copy() -> TestResult {
    // Given: an unbooted managed system and a nonzero, unaligned-length payload.
    let payload = b"firmware-managed-copy";
    // When: one Host program probes closed access, waits for firmware, then submits DMA.
    let report = harness::run(Input::Copy(payload)).await?;
    let outcome = report.program?;
    // Then: work was blocked, firmware alone opened access, and the ISR consumed the copy response.
    assert_eq!(outcome.before.read, Err(AccessError::Closed));
    assert_eq!(outcome.before.write, Err(AccessError::Closed));
    assert_eq!(outcome.before.control_read, Err(AccessError::Denied));
    assert_eq!(outcome.before.control_write, Err(AccessError::Denied));
    assert_eq!(outcome.before.output, [0xa5; 4]);
    assert_eq!(outcome.before.ready, 0);
    for rejected in outcome.after {
        assert_eq!(rejected, Err(AccessError::Denied));
    }
    assert_eq!(outcome.completion, 37);
    let mut copied = vec![0; payload.len()];
    report
        .storage
        .read(outcome.destination, MemoryAlignment::ONE, &mut copied)?;
    assert_eq!(copied, payload);
    assert!(report.summary.deliveries() >= 3);
    assert_eq!(outcome.dma_interrupts, 1);
    assert_eq!(outcome.released, 0);
    assert_eq!(Arc::strong_count(&report.storage), 1);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn early_host_error_cancels_all_scoped_system_owners() -> TestResult {
    // Given: a Host program that intentionally exits after firmware readiness.
    // When: execution ends with its original error instead of submitting a command.
    let report = harness::run(Input::StopAfterReady).await?;
    // Then: the error survives and no hardware, firmware or Host owner retains backing memory.
    let error = report.program.err().ok_or("Host unexpectedly succeeded")?;
    let stopped = error
        .downcast_ref::<std::io::Error>()
        .ok_or("Host error changed type")?;
    assert_eq!(stopped.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(report.summary.deliveries(), 0);
    assert!(report.storage.regions()?.is_empty());
    assert_eq!(Arc::strong_count(&report.storage), 1);
    Ok(())
}
