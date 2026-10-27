//! System acceptance for batched notifications and software channel recovery.

mod hardware;
mod software;

use crate::harness::TestResult;
use hardware_interface_memory::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::dma::{ChannelFatalCode, Event, EventCode};
use pdma_model_contract::dma::{ErrorInfo, EventInfoValues};

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Batch,
    ChannelReset,
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_batch_fence_and_trap_notify_once_without_events() -> TestResult {
    // Given: three copies followed by one Fence and one Trap.
    let payload = b"batch-copy";
    // When: the complete software program submits and drains through its registered ISR.
    let report = hardware::run(Scenario::Batch, payload).await?;
    // Then: one notification exposes the Fence and all copied bytes without error events.
    assert_eq!(report.summary.deliveries(), 1);
    assert!(report.worker.is_cancelled());
    assert!(report.outcome.events.is_empty());
    assert_eq!(report.outcome.completed, 42);
    assert_eq!(report.outcome.causes.len(), 1);
    let cause = report.outcome.causes.first().ok_or("missing batch cause")?;
    assert!(!cause.event_non_empty());
    assert!(cause.trap());
    assert!(!cause.error());
    let mut copied = vec![0; payload.len() * 3];
    report.storage.read(
        report.outcome.destination,
        MemoryAlignment::ONE,
        &mut copied,
    )?;
    assert_eq!(copied, payload.repeat(3));
    assert!(!report.irq);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_error_irq_allows_channel_reset_and_next_copy_in_same_runtime() -> TestResult {
    // Given: a length-mismatched command with both routine notification kinds masked.
    let payload = b"after-reset";
    // When: software handles the error, resets, rebuilds its driver and submits a valid copy.
    let report = hardware::run(Scenario::ChannelReset, payload).await?;
    // Then: the error bypassed masks and a fresh nonempty event completed the next copy.
    assert_eq!(report.summary.deliveries(), 2);
    assert!(report.worker.is_cancelled());
    assert_eq!(
        report.outcome.events,
        vec![Event {
            id: 41,
            code: EventCode::LengthMismatch,
            info: EventInfoValues::LengthMismatch(ErrorInfo { bytes_completed: 0 })
        }]
    );
    assert_eq!(report.outcome.fatal, ChannelFatalCode::None);
    assert_eq!(report.outcome.causes.len(), 2);
    let first = report.outcome.causes.first().ok_or("missing error cause")?;
    assert!(first.error());
    let second = report
        .outcome
        .causes
        .get(1)
        .ok_or("missing recovered cause")?;
    assert!(!second.event_non_empty());
    assert!(!second.error());
    assert!(second.trap());
    assert_eq!(report.outcome.completed, 42);
    let mut copied = vec![0; payload.len()];
    report.storage.read(
        report.outcome.destination,
        MemoryAlignment::ONE,
        &mut copied,
    )?;
    assert_eq!(copied, payload);
    assert!(!report.irq);
    Ok(())
}
